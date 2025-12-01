#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use userland::canon;
use userland::prelude::*;
use userland::uuid::Uuid;
use userland::{AbiRequest, Symbol};

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

    // Create /icons
    let icons_id = create_directory("icons", Some(root_id));
    userland::println!("Created /icons: {:?}", icons_id);

    // Populate /bin
    for app in userland::apps_manifest::APPS {
        create_app_file(app, bin_id);
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
    fields.insert(canon::SHOW_IN_LAUNCHER, Value::Bool(app.show_in_launcher));

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

fn create_icon_file(name: &str, parent: Uuid, color: u32) {
    let mut fields = userland::map();
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::KIND, Value::Symbol(canon::FILE));
    fields.insert(canon::PARENT, Value::Uuid(parent));
    fields.insert(canon::MIME, Value::Text("image/raw-argb".into()));

    // Generate 32x32 icon data
    let width = 32;
    let height = 32;
    let mut data = vec![0u8; width * height * 4];
    for i in 0..width * height {
        data[i * 4] = (color >> 16) as u8; // B
        data[i * 4 + 1] = (color >> 8) as u8; // G
        data[i * 4 + 2] = color as u8; // R
        data[i * 4 + 3] = (color >> 24) as u8; // A
    }

    // Add a simple pattern (border)
    for x in 0..width {
        let offset = (x * 4) as usize;
        data[offset] = 0;
        data[offset + 1] = 0;
        data[offset + 2] = 0; // Top
        let offset = ((height - 1) * width + x) as usize * 4;
        data[offset] = 0;
        data[offset + 1] = 0;
        data[offset + 2] = 0; // Bottom
    }
    for y in 0..height {
        let offset = (y * width) as usize * 4;
        data[offset] = 0;
        data[offset + 1] = 0;
        data[offset + 2] = 0; // Left
        let offset = (y * width + width - 1) as usize * 4;
        data[offset] = 0;
        data[offset + 1] = 0;
        data[offset + 2] = 0; // Right
    }

    fields.insert(canon::BYTES, Value::Bytes(data));
    fields.insert(canon::WIDTH, Value::U64(width as u64));
    fields.insert(canon::HEIGHT, Value::U64(height as u64));

    let file_id = userland::fiat(None, canon::FILE, fields);

    link(parent, "contains", file_id);
    link(file_id, "parent", parent);
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
