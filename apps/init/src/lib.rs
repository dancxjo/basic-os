#![cfg_attr(not(feature = "std"), no_std)]
use userland::canon;
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

    // Spawn compositor
    userland::println!("Spawning compositor...");
    userland::sys::spawn("compositor");

    // Spawn demo app
    userland::println!("Spawning demo_app...");
    userland::sys::spawn("demo_app");

    // Spawn self_editing_demo
    userland::println!("Spawning self_editing_demo...");
    userland::sys::spawn("self_editing_demo");

    // Spawn graph_viewer
    userland::println!("Spawning graph_viewer...");
    userland::sys::spawn("graph_viewer");

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
