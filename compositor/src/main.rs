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
    canon, drivers, emit_frame_ready, fiat, map, println, that, DriverContext, FramebufferDriver,
    KeyboardDriver, MouseDriver, SerialDriver, Value,
};
use uuid::Uuid;

const FRAME_INTERVAL_SPINS: usize = 10_000_000;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    heap::init_heap();

    drivers::register_builtin_drivers();
    drivers::connect_stream("ps2-keyboard", compositor_id(), 0);
    drivers::connect_stream("ps2-mouse", compositor_id(), 0);
    drivers::connect_stream("limine-framebuffer", framebuffer_id(), 0);
    register_compositor_things();
    let mut apps = register_apps();

    let mut driver_ctx = DriverContext::new();
    let mut keyboard = KeyboardDriver::init(&mut driver_ctx);
    let mut mouse = MouseDriver::init(&mut driver_ctx);
    let mut _framebuffer_driver = FramebufferDriver::init(&mut driver_ctx);
    let _serial_driver = SerialDriver::init(&mut driver_ctx);

    let mut compositor = Compositor::new();
    let mut tick: u64 = 0;
    loop {
        if let Some(kb) = keyboard.as_mut() {
            kb.poll(&mut driver_ctx);
        }
        if let Some(ms) = mouse.as_mut() {
            ms.poll(&mut driver_ctx);
        }
        tick_apps(&mut apps, tick);
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

fn register_apps() -> Vec<DynApp> {
    vec![
        clouds_app(compositor_id()),
        hello_app(compositor_id()),
        clock_app(compositor_id()),
        graph_demo_app(compositor_id()),
    ]
}

fn tick_apps(apps: &mut [DynApp], tick: u64) {
    for app in apps.iter_mut() {
        app.tick(tick);
    }
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
