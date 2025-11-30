use std::thread;
use std::time::Duration;

use app_clouds::CloudsApp;
use thing_host::HostRuntime;
use userland::app;
use userland::watch::WatchManager;
use uuid::Uuid;

fn main() {
    let runtime = Box::leak(Box::new(HostRuntime::new()));
    userland::set_runtime(runtime);

    let mut watch_manager = WatchManager::new();
    let compositor_id = Uuid::nil();
    let mut app_runner = app::create_app::<CloudsApp>(compositor_id, &mut watch_manager);

    let mut tick: u64 = 0;
    loop {
        let app_id = app_runner.app_id();
        watch_manager.process_graph(&[app_id]);
        app_runner.tick(&mut watch_manager, tick);
        tick = tick.wrapping_add(1);
        thread::sleep(Duration::from_millis(33));
    }
}
