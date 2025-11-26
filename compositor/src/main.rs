#![no_std]
#![no_main]

extern crate alloc;

mod heap;

use alloc::vec;
use alloc::vec::Vec;
use app_clock::app_entry as clock_app;
use app_hello::app_entry as hello_app;
use app_keyboard_driver::app_entry as keyboard_driver_app;
use app_framebuffer_driver::app_entry as framebuffer_driver_app;
use compositor::{Compositor, FramebufferInfo, FramebufferTarget};
use userland::app::DynApp;
use userland::{canon, drivers, fiat, graph_snapshot, map, println, that, Value, WatchManager};
use uuid::Uuid;

const FRAME_INTERVAL_SPINS: usize = 10_000_000;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    heap::init_heap();

    drivers::register_builtin_drivers();
    // register_compositor_things();
    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();
    let mut apps = register_apps(&mut watch_manager);

    let fb_target = discover_framebuffer().unwrap_or_else(fallback_framebuffer);
    println!(
        "Framebuffer info: {}x{} pitch={} bpp={}",
        fb_target.info.width, fb_target.info.height, fb_target.info.pitch, fb_target.info.bpp
    );

    let mut compositor =
        Compositor::init_with_watches(&mut watch_manager, compositor_app_id, fb_target);
    let mut tick: u64 = 0;
    loop {
        let mut all_ids = vec![compositor_app_id];
        all_ids.extend(app_ids(&apps));

        watch_manager.process_graph(&all_ids);

        for ev in watch_manager.drain_inbox(compositor_app_id) {
            compositor.on_event(&ev);
        }

        tick_apps(&mut apps, &mut watch_manager, tick);
        compositor.tick();
        tick = tick.wrapping_add(1);
        busy_wait();
    }
}

fn register_compositor_things() {
    let compositor_id = compositor_id();
    let surface_id = compositor_surface_id();

    let mut compositor_fields = map();
    compositor_fields.insert(canon::NAME, Value::text("compositor0"));
    compositor_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(compositor_id), canon::COMPOSITOR, compositor_fields);

    let mut surface_fields = map();
    surface_fields.insert(canon::NAME, Value::text("compositor-surface"));
    surface_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(surface_id), canon::PIXMAP, surface_fields);
}

fn register_apps(watch_manager: &mut WatchManager) -> Vec<DynApp> {
    vec![
        clock_app(compositor_id(), watch_manager),
        hello_app(compositor_id(), watch_manager),
        keyboard_driver_app(compositor_id(), watch_manager),
        framebuffer_driver_app(compositor_id(), watch_manager),
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

fn discover_framebuffer() -> Option<FramebufferTarget> {
    let info = userland::sys::fb_info()?;
    Some(FramebufferTarget {
        info: FramebufferInfo {
            width: info.width as usize,
            height: info.height as usize,
            pitch: info.pitch as usize,
            bpp: info.bpp as u16,
        },
        addr: core::ptr::null_mut(), // Not used by compositor anymore
        len_bytes: 0,
    })
}

fn fallback_framebuffer() -> FramebufferTarget {
    FramebufferTarget {
        info: FramebufferInfo {
            width: 1024,
            height: 768,
            pitch: 1024 * 4,
            bpp: 32,
        },
        addr: core::ptr::null_mut(),
        len_bytes: 0,
    }
}

fn compositor_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, b"compositor0")
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
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic inside userland compositor: {info}");
    loop {}
}
