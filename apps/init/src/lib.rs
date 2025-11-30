#![cfg_attr(not(feature = "std"), no_std)]
use userland::canon;
use userland::fs::{self, FsKind};
use userland::prelude::*;

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

    // Sync launcher widgets from /bin
    userland::println!("Syncing launcher widgets...");
    
    // Give compositor time to start watching
    for _ in 0..5000000 {
        core::hint::spin_loop();
    }

    if let Err(e) = userland::launcher::sync_launcher_from_bin() {
        userland::println!("Failed to sync launcher: {:?}", e);
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
