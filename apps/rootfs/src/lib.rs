#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use userland::canon;
use userland::prelude::*;
use userland::uuid::Uuid;
use userland::{AbiRequest, Symbol};

// Define symbols locally for now
// const DIRECTORY: Symbol = canon::canon(b'D', b'I', b'R');
// const FILE: Symbol = canon::canon(b'F', b'I', b'L');
// const DEVICE: Symbol = canon::canon(b'D', b'E', b'V');

pub fn app_main() -> ! {
    userland::println!("RootFS service started.");

    // Create root directory "/"
    let root_id = create_directory("/", None);
    userland::println!("Created root directory: {:?}", root_id);

    // Create /bin
    let bin_id = create_directory("bin", Some(root_id));
    userland::println!("Created /bin: {:?}", bin_id);

    // Create /dev
    let dev_id = create_directory("dev", Some(root_id));
    userland::println!("Created /dev: {:?}", dev_id);

    // Create /tmp
    let tmp_id = create_directory("tmp", Some(root_id));
    userland::println!("Created /tmp: {:?}", tmp_id);

    // Populate /bin
    let apps = [
        "init",
        "compositor",
        "demo_app",
        "graph_viewer",
        "text_editor",
        "self_editing_demo",
    ];

    for app in apps {
        create_file(app, bin_id);
    }

    // Populate /dev
    create_device("tty0", dev_id);

    userland::println!("RootFS initialization complete. Entering idle loop.");

    // Just sleep/idle
    loop {
        core::hint::spin_loop();
    }
}

fn create_directory(name: &str, parent: Option<Uuid>) -> Uuid {
    let id = userland::simple_uuid(name.as_bytes()); 
    
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::DIRECTORY));
    if let Some(p) = parent {
        fields.insert(canon::PARENT, Value::Uuid(p));
    }

    let dir_id = userland::fiat(Some(id), canon::DIRECTORY, fields);

    if let Some(parent_id) = parent {
        link(parent_id, "contains", dir_id);
        link(dir_id, "parent", parent_id);
    }

    dir_id
}

fn create_file(name: &str, parent: Uuid) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::FILE));
    fields.insert(canon::PARENT, Value::Uuid(parent));
    
    // Bundle ID (using simple_uuid of name as placeholder)
    let bundle_id = userland::simple_uuid(name.as_bytes());
    fields.insert(canon::BUNDLE_ID, Value::Uuid(bundle_id));

    // Maybe add executable info
    fields.insert(canon::canon(b'E', b'X', b'E'), Value::Text(name.into()));

    let file_id = userland::fiat(None, canon::FILE, fields);

    link(parent, "contains", file_id);
    link(file_id, "parent", parent);
}

fn create_device(name: &str, parent: Uuid) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::DEVICE));
    fields.insert(canon::PARENT, Value::Uuid(parent));
    
    if name == "tty0" {
        fields.insert(canon::DEVICE_DRIVER, Value::Text("console".into()));
        fields.insert(canon::DEVICE_ID, Value::Text("tty0".into()));
    }

    let dev_id = userland::fiat(None, canon::DEVICE, fields);

    link(parent, "contains", dev_id);
    link(dev_id, "parent", parent);
}

fn link(from: Uuid, pred: &str, to: Uuid) {
    let req = AbiRequest::Link {
        id: None,
        from,
        pred: pred.to_string(),
        to,
        props: userland::map(),
    };
    let _ = userland::runtime().call(req);
}
