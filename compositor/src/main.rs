#![no_std]
#![no_main]

extern crate alloc;

mod heap;

use alloc::vec;
use alloc::vec::Vec;
use app_clock::app_entry as clock_app;
use app_clouds::app_entry as clouds_app;
use app_graph_demo::app_entry as graph_demo_app;
use app_hello::app_entry as hello_app;
use compositor::Compositor;
use userland::app::DynApp;
use userland::{
    canon, drivers, emit_frame_ready, fetch_journal_events, fiat, map, println,
    start_builtin_drivers, that, DriverContext, Value, WatchManager,
};
use uuid::Uuid;

const FRAME_INTERVAL_SPINS: usize = 10_000_000;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    heap::init_heap();

    drivers::register_builtin_drivers();
    register_compositor_things();
    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();
    let mut apps = register_apps(&mut watch_manager);

    let mut driver_ctx = DriverContext::new();
    let mut running_drivers =
        start_builtin_drivers(&mut driver_ctx, compositor_id(), framebuffer_id());

    let mut compositor = Compositor::init_with_watches(&mut watch_manager, compositor_app_id);
    let mut tick: u64 = 0;
    loop {
        running_drivers.poll_all(&mut driver_ctx);
        let journal_events = fetch_journal_events().unwrap_or_default();

        let mut all_ids = vec![compositor_app_id];
        all_ids.extend(app_ids(&apps));

        watch_manager.process_journal_batch(&all_ids, &journal_events);
        watch_manager.process_graph(&all_ids);

        for ev in watch_manager.drain_inbox(compositor_app_id) {
            compositor.on_event(&ev);
        }

        tick_apps(&mut apps, &mut watch_manager, tick);
        let frame = compositor.tick();
        emit_frame_ready(
            compositor_id(),
            framebuffer_id(),
            compositor_surface_id(),
            frame.as_bytes(),
        );
        tick = tick.wrapping_add(1);
        busy_wait();
    }
}

fn register_compositor_things() {
    let compositor_id = compositor_id();
    let framebuffer_id = framebuffer_id();
    let surface_id = compositor_surface_id();

    let mut compositor_fields = map();
    compositor_fields.insert(canon::NAME, Value::text("compositor0"));
    compositor_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(compositor_id), canon::COMPOSITOR, compositor_fields);

    let mut fb_fields = map();
    fb_fields.insert(canon::NAME, Value::text("framebuffer0"));
    fb_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(framebuffer_id), canon::PIXMAP, fb_fields);

    let mut surface_fields = map();
    surface_fields.insert(canon::NAME, Value::text("compositor-surface"));
    surface_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(surface_id), canon::PIXMAP, surface_fields);

    that(compositor_id, canon::STREAMS, framebuffer_id, 0);
}

fn register_apps(watch_manager: &mut WatchManager) -> Vec<DynApp> {
    vec![
        clouds_app(compositor_id(), watch_manager),
        hello_app(compositor_id(), watch_manager),
        clock_app(compositor_id(), watch_manager),
        graph_demo_app(compositor_id(), watch_manager),
    ]
}

fn tick_apps(apps: &mut [DynApp], watch_manager: &mut WatchManager, tick: u64) {
    for app in apps.iter_mut() {
        app.tick(watch_manager, tick);
    }
}

fn app_ids(apps: &[DynApp]) -> Vec<usize> {
    apps.iter().map(|app| app.app_id()).collect()
}

fn compositor_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, b"compositor0")
}

fn framebuffer_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, b"framebuffer0")
}

fn compositor_surface_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, b"compositor-surface0")
}

fn busy_wait() {
    for _ in 0..FRAME_INTERVAL_SPINS {
        core::hint::spin_loop();
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic inside userland compositor!");
    loop {}
}
