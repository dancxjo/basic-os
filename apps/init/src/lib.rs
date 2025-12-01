#![cfg_attr(not(feature = "std"), no_std)]
use userland::canon;
use userland::fs::{self, FsKind};
use userland::prelude::*;
use userland::uuid::Uuid;

pub fn app_main() -> ! {
    userland::println!("Init process started.");

    // Spawn drivers
    userland::println!("Spawning framebuffer_driver...");
    userland::sys::spawn("framebuffer_driver");

    userland::println!("Spawning keyboard_driver...");
    userland::sys::spawn("keyboard_driver");

    userland::println!("Spawning mouse_driver...");
    userland::sys::spawn("mouse_driver");

    // Spawn rootfs
    userland::println!("Spawning rootfs...");
    userland::sys::spawn("rootfs");

    // Wait a bit for rootfs to populate /bin
    // In a real system we would watch for /bin changes or have a dependency graph.
    // For now, just sleep a bit or retry.
    // userland::sys::sleep(500); // sleep not implemented yet, spin loop

    let mut retries = 0;
    loop {
        userland::println!(
            "Scanning /bin for autostart apps (attempt {})...",
            retries + 1
        );
        if let Ok(entries) = fs::read_dir("/bin") {
            if !entries.is_empty() {
                for node in entries {
                    if node.kind == FsKind::File && node.autostart {
                        if let Some(bin_name) = &node.bin_name {
                            userland::println!("Autostarting {}...", node.name);
                            userland::sys::spawn(bin_name);
                        }
                    }
                }
                break;
            }
        }

        retries += 1;
        if retries > 10 {
            userland::println!("Failed to read /bin or empty after retries");
            break;
        }

        // Spin for a bit
        for _ in 0..1000000 {
            core::hint::spin_loop();
        }
    }

    // Create toolbar
    create_toolbar();

    // Give compositor time to start watching
    for _ in 0..5000000 {
        core::hint::spin_loop();
    }

    // Create LaunchRequest for text_editor
    userland::println!("Creating LaunchRequest for text_editor...");
    let req_id = userland::simple_uuid(b"LaunchTextEditor");
    let mut fields = userland::map();
    fields.insert(canon::PACKAGE, Value::Text("text_editor".into()));
    fields.insert(canon::NAME, Value::Text("Launch Text Editor".into()));
    userland::fiat(Some(req_id), canon::LAUNCH_REQUEST, fields);

    userland::println!("Init sequence complete. Entering idle loop.");

    let mut watch_manager = WatchManager::new();
    let init_app_id = watch_manager.register_app();
    let _watch_id = watch_manager.register_graph(
        init_app_id,
        userland::ThingFilter {
            kind: Some(canon::LAUNCH_REQUEST),
            id: None,
        },
    );

    loop {
        watch_manager.process_graph(&[init_app_id]);
        for ev in watch_manager.drain_inbox(init_app_id) {
            if let AppEvent::Thing { thing, .. } = ev {
                if thing.kind == canon::LAUNCH_REQUEST {
                    // Check if status is INIT (to avoid re-processing if we update it)
                    let status = thing.fields.get(&canon::STATUS).and_then(|v| v.as_symbol());
                    if status == Some(canon::DONE) {
                        continue;
                    }

                    if let Some(pkg) = thing.fields.get(&canon::PACKAGE).and_then(|v| v.as_text()) {
                        userland::println!("Handling LaunchRequest for {}", pkg);
                        userland::sys::spawn(pkg);

                        // Mark as DONE
                        let mut updates = userland::map();
                        updates.insert(canon::STATUS, Value::Symbol(canon::DONE));
                        userland::fiat(Some(thing.id), canon::LAUNCH_REQUEST, updates);
                    }
                }
            }
        }
    }
}

fn create_toolbar() {
    userland::println!("Creating toolbar...");

    // Toolbar container
    let toolbar_id = userland::simple_uuid(b"Toolbar");
    let mut fields = userland::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(canon::cc('W', 'K'), Value::Text("toolbar".into()));
    fields.insert(canon::WIDTH, Value::U64(800)); // Screen width?
    fields.insert(canon::HEIGHT, Value::U64(40));
    fields.insert(canon::X, Value::I64(0));
    fields.insert(canon::Y, Value::I64(0)); // Top
    userland::fiat(Some(toolbar_id), canon::WIDGET, fields);

    // Button 1: Clouds
    create_toolbar_button(toolbar_id, "Clouds", "demo_app", "clouds", 0);

    // Button 2: Text Editor
    create_toolbar_button(toolbar_id, "Text", "text_editor", "text", 1);

    // Button 3: Graph Viewer
    create_toolbar_button(toolbar_id, "Graph", "graph_viewer", "graph", 2);
}

fn create_toolbar_button(_parent: Uuid, label: &str, target: &str, icon_path: &str, index: i32) {
    let id = userland::simple_uuid(label.as_bytes()); // Simple ID generation
    let mut fields = userland::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(canon::cc('W', 'K'), Value::Text("toolbar_button".into()));
    fields.insert(canon::WIDTH, Value::U64(60));
    fields.insert(canon::HEIGHT, Value::U64(30));
    fields.insert(canon::ICON_NAME, Value::Text(icon_path.into()));

    // Position relative to toolbar? Or absolute?
    // If absolute, we need to know toolbar position.
    // For now, let's put them inside the toolbar area visually.
    // Assuming toolbar is at 0,0.
    let x = 10 + index * 70;
    let y = 5;

    fields.insert(canon::X, Value::I64(x as i64));
    fields.insert(canon::Y, Value::I64(y as i64));

    fields.insert(canon::TEXT, Value::Text(label.into()));
    fields.insert(canon::TARGET, Value::Text(target.into()));

    userland::fiat(Some(id), canon::WIDGET, fields);
}
