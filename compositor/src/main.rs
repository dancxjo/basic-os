#![no_std]
#![no_main]

extern crate alloc;

mod heap;

use app_clock::{register as register_clock, tick as tick_clock, AppHandle as ClockHandle};
use app_clouds::{register as register_clouds, tick as tick_clouds, AppHandle as CloudsHandle};
use app_hello::{register as register_hello, tick as tick_hello, AppHandle as HelloHandle};
use compositor::Compositor;
use userland::{canon, emit_edge_added, emit_frame_ready, emit_thing_created, map, println, Value};
use uuid::Uuid;

const FRAME_INTERVAL_SPINS: usize = 10_000_000;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    heap::init_heap();

    register_compositor_things();
    let apps = register_apps();

    let mut compositor = Compositor::new();
    let mut tick: u64 = 0;
    loop {
        tick_apps(&apps, tick);
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
    emit_thing_created(compositor_id, canon::COMPOSITOR, 0, compositor_fields);

    let mut fb_fields = map();
    fb_fields.insert(canon::NAME, Value::text("framebuffer0"));
    fb_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    emit_thing_created(framebuffer_id, canon::PIXMAP, 0, fb_fields);

    let mut surface_fields = map();
    surface_fields.insert(canon::NAME, Value::text("compositor-surface"));
    surface_fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    emit_thing_created(surface_id, canon::PIXMAP, 0, surface_fields);

    emit_edge_added(compositor_id, canon::STREAMS, framebuffer_id, 0);
}

struct Apps {
    clouds: CloudsHandle,
    hello: HelloHandle,
    clock: ClockHandle,
}

fn register_apps() -> Apps {
    Apps {
        clouds: register_clouds(compositor_id()),
        hello: register_hello(compositor_id()),
        clock: register_clock(compositor_id()),
    }
}

fn tick_apps(apps: &Apps, tick: u64) {
    tick_clouds(&apps.clouds, tick);
    tick_hello(&apps.hello, tick);
    tick_clock(&apps.clock, tick);
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
