use crate::{canon, prelude::*, AbiRequest, NodePattern, Symbol, Value};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsKind {
    Dir,
    File,
    CharDevice,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FsNode {
    pub id: Uuid,     // or Uuid, whichever you use
    pub name: String, // basename, not full path
    pub kind: FsKind,
    // Optional metadata for future use:
    pub mode: Option<u32>,
    pub owner_bundle: Option<Uuid>,

    // For executable files:
    pub bundle_id: Option<Uuid>, // when this is a "bundle-backed" file
    pub bin_name: Option<String>,
    pub autostart: bool,
    pub show_in_launcher: bool,

    // For char devices:
    pub device_driver: Option<String>, // e.g. "console"
    pub device_id: Option<String>,     // e.g. "tty0"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    NotADirectory,
    IOError,
}

pub fn find_root() -> Result<Uuid, FsError> {
    // Root has a deterministic ID based on name "/"
    Ok(crate::simple_uuid(b"/"))
}

pub fn lookup_path(path: &str) -> Result<FsNode, FsError> {
    let root_id = find_root()?;

    if path == "/" {
        return get_node_by_id(root_id);
    }

    let mut current_id = root_id;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for (i, part) in parts.iter().enumerate() {
        // Find child with name == part and parent == current_id
        let mut props = crate::map();
        props.insert(canon::PARENT, Value::Uuid(current_id));
        props.insert(canon::NAME, Value::Text((*part).into()));

        let pattern = NodePattern {
            labels: Vec::new(),
            props,
        };

        let things = crate::graph::get_nodes(pattern);
        if let Some(thing) = things.first() {
            current_id = thing.id;
            if i == parts.len() - 1 {
                return thing_to_fs_node(thing);
            }
        } else {
            return Err(FsError::NotFound);
        }
    }

    // Should be unreachable if path is not empty and not "/"
    // But if path was empty string?
    if parts.is_empty() {
        return get_node_by_id(root_id);
    }

    Err(FsError::NotFound)
}

pub fn read_dir(path: &str) -> Result<Vec<FsNode>, FsError> {
    let dir_node = lookup_path(path)?;
    if dir_node.kind != FsKind::Dir {
        return Err(FsError::NotADirectory);
    }

    let mut props = crate::map();
    props.insert(canon::PARENT, Value::Uuid(dir_node.id));

    let pattern = NodePattern {
        labels: Vec::new(),
        props,
    };

    let things = crate::graph::get_nodes(pattern);
    let mut nodes = Vec::new();
    for thing in things {
        if let Ok(node) = thing_to_fs_node(&thing) {
            nodes.push(node);
        }
    }

    // Sort by name
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(nodes)
}

fn get_node_by_id(id: Uuid) -> Result<FsNode, FsError> {
    let req = AbiRequest::Get { id };
    match crate::runtime().call(req) {
        crate::AbiResponse::Get { thing } => {
            if let Some(t) = thing {
                thing_to_fs_node(&t)
            } else {
                Err(FsError::NotFound)
            }
        }
        _ => Err(FsError::IOError),
    }
}

fn thing_to_fs_node(thing: &crate::GraphThing) -> Result<FsNode, FsError> {
    let name = thing
        .fields
        .get(&canon::NAME)
        .and_then(|v| extract_text(v))
        .unwrap_or_else(|| "unknown".to_string());

    let kind_sym = thing
        .fields
        .get(&canon::KIND)
        .and_then(|v| v.as_symbol())
        .unwrap_or(Symbol(0));

    let kind = if kind_sym == canon::DIRECTORY {
        FsKind::Dir
    } else if kind_sym == canon::FILE {
        FsKind::File
    } else if kind_sym == canon::DEVICE {
        FsKind::CharDevice
    } else {
        FsKind::Unknown
    };

    let bundle_id = thing
        .fields
        .get(&canon::BUNDLE_ID)
        .and_then(|v| v.as_uuid());

    let bin_name = thing
        .fields
        .get(&canon::BIN_NAME)
        .and_then(|v| extract_text(v));

    let autostart = thing
        .fields
        .get(&canon::AUTOSTART)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let show_in_launcher = thing
        .fields
        .get(&canon::SHOW_IN_LAUNCHER)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let device_driver = thing
        .fields
        .get(&canon::DEVICE_DRIVER)
        .and_then(|v| extract_text(v));

    let device_id = thing
        .fields
        .get(&canon::DEVICE_ID)
        .and_then(|v| extract_text(v));

    Ok(FsNode {
        id: thing.id,
        name,
        kind,
        mode: None,
        owner_bundle: Some(thing.owner),
        bundle_id,
        bin_name,
        autostart,
        show_in_launcher,
        device_driver,
        device_id,
    })
}
