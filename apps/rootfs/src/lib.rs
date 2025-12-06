#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use userland::canon;
use userland::prelude::*;
use userland::uuid::Uuid;
use userland::{AbiRequest, Symbol};

const NOTO_SANS_SYMBOLS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/NotoSansSymbols-Regular.ttf"));
const NOTO_SANS_SYMBOLS_2: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/NotoSansSymbols2-Regular.ttf"));

pub fn app_main() -> ! {
    init();
    userland::println!("RootFS initialization complete. Entering idle loop.");

    // Just sleep/idle
    loop {
        core::hint::spin_loop();
    }
}

pub fn init() {
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

    // Create /icons
    let icons_id = create_directory("icons", Some(root_id));
    userland::println!("Created /icons: {:?}", icons_id);

    // Create /fonts
    let fonts_id = create_directory("fonts", Some(root_id));
    userland::println!("Created /fonts: {:?}", fonts_id);

    // Create /places
    let places_id = create_directory("places", Some(root_id));
    userland::println!("Created /places: {:?}", places_id);

    // Create /places/sky
    create_place("sky", places_id, "graph_viewer");

    create_font_file(
        "NotoSansSymbols-Regular.ttf",
        fonts_id,
        "font/ttf",
        NOTO_SANS_SYMBOLS,
    );
    create_font_file(
        "NotoSansSymbols2-Regular.ttf",
        fonts_id,
        "font/ttf",
        NOTO_SANS_SYMBOLS_2,
    );

    // Load assets from boot module via graph
    let boot_assets = userland::find_by_kind("boot.asset");
    let mut found_assets = false;
    for asset in boot_assets {
        if let Some(userland::Value::Bytes(assets_tar)) = asset.fields.get(&canon::BYTES) {
             userland::println!("Found assets.tar via graph, size: {}", assets_tar.len());
             load_assets_from_tar(&assets_tar, icons_id);
             found_assets = true;
             break;
        }
    }
    if !found_assets {
        userland::println!("assets.tar not found in graph!");
    }

    // Populate /bin
    for app in userland::apps_manifest::APPS {
        create_app_file(app, bin_id);
    }

    // Populate /dev
    create_device("tty0", dev_id);
}

fn create_place(name: &str, parent: Uuid, app_name: &str) -> Uuid {
    let id = userland::simple_uuid(name.as_bytes());

    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::PLACE));
    fields.insert(canon::APP, Value::Text(app_name.into()));
    fields.insert(canon::PARENT, Value::Uuid(parent));

    let place_id = userland::fiat(Some(id), canon::PLACE, fields);

    link(parent, "contains", place_id);
    link(place_id, "parent", parent);

    place_id
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

fn create_app_file(app: &userland::apps_manifest::AppSpec, parent: Uuid) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(app.name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::FILE));
    fields.insert(canon::PARENT, Value::Uuid(parent));

    // Bundle ID (using simple_uuid of name as placeholder)
    let bundle_id = userland::simple_uuid(app.name.as_bytes());
    fields.insert(canon::BUNDLE_ID, Value::Uuid(bundle_id));

    // Metadata
    fields.insert(canon::BIN_NAME, Value::Text(app.bin_name.into()));
    fields.insert(canon::AUTOSTART, Value::Bool(app.autostart));
    fields.insert(
        canon::SHOW_IN_GRAPH_VIEWER,
        Value::Bool(app.show_in_graph_viewer),
    );

    // Maybe add executable info
    fields.insert(canon::canon(b'E', b'X', b'E'), Value::Text(app.name.into()));

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

fn create_font_file(name: &str, parent: Uuid, mime: &str, data: &[u8]) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::FILE));
    fields.insert(canon::PARENT, Value::Uuid(parent));
    fields.insert(canon::MIME, Value::Text(mime.into()));
    fields.insert(canon::BYTES, Value::Bytes(data.to_vec()));
    fields.insert(canon::LENGTH, Value::U64(data.len() as u64));

    let file_id = userland::fiat(None, canon::FILE, fields);
    link(parent, "contains", file_id);
    link(file_id, "parent", parent);
}

fn load_assets_from_tar(tar_data: &[u8], icons_dir_id: Uuid) {
    let mut offset = 0;
    while offset + 512 <= tar_data.len() {
        let header = &tar_data[offset..offset + 512];
        
        // Check if ends
        if header.iter().all(|&b| b == 0) {
            break; 
        }

        let name_bytes = &header[0..100];
        let name_str = core::str::from_utf8(name_bytes)
            .unwrap_or("")
            .trim_matches('\0');

        let size_str = core::str::from_utf8(&header[124..136])
            .unwrap_or("")
            .trim_matches('\0')
            .trim();
        let size = u64::from_str_radix(size_str, 8).unwrap_or(0);
        let type_flag = header[156];

        let content_start = offset + 512;
        let content_end = content_start + size as usize;

        if type_flag == b'0' || type_flag == 0 {
            // Normal file
            if content_end > tar_data.len() {
               break; // Truncated
            }
            let data = &tar_data[content_start..content_end];
            
            if name_str.starts_with("icons/") && name_str.ends_with(".svg") {
                let file_name = name_str.strip_prefix("icons/").unwrap();
                // Strip extension for resource name if desired, or keep it.
                // Keeping it logic simple for now. 
                // The widget_icon system expects names like "home", "settings" etc.
                // The file names are "home.svg", so we strip .svg
                let resource_name = file_name.trim_end_matches(".svg");
                create_asset_file(resource_name, icons_dir_id, "image/svg+xml", data);
            }
        }

        // Move to next block, aligned to 512
        offset = (content_end + 511) & !511;
    }
}

fn create_asset_file(name: &str, parent: Uuid, mime: &str, data: &[u8]) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::FILE));
    fields.insert(canon::PARENT, Value::Uuid(parent));
    fields.insert(canon::MIME, Value::Text(mime.into()));
    fields.insert(canon::BYTES, Value::Bytes(data.to_vec()));
    
    // Also insert ICON_NAME so lookups by icon name work directly if we query files
    fields.insert(canon::ICON_NAME, Value::Text(name.into()));

    let file_id = userland::fiat(None, canon::FILE, fields);
    link(parent, "contains", file_id);
    link(file_id, "parent", parent);
    
    userland::println!("Created asset: {} ({})", name, mime);
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
