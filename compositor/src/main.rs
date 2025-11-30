#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;

use compositor::{Compositor, FramebufferBackend, FramebufferInfo, FramebufferTarget};
use userland::{println, WatchManager};

const FRAME_INTERVAL_SPINS: usize = 10_000_000;
static mut BACKBUFFER_STORAGE: [u32; 8_388_608] = [0; 8_388_608];

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    userland::init_heap();

    // drivers::register_builtin_drivers(); // Drivers are now separate processes
    // register_compositor_things();
    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();

    // No more register_apps() - they are separate processes

    let fb_target = discover_framebuffer().unwrap_or_else(fallback_framebuffer);
    println!(
        "Framebuffer info: {}x{} pitch={} bpp={}",
        fb_target.info.width, fb_target.info.height, fb_target.info.pitch, fb_target.info.bpp
    );

    let backend = unsafe {
        FramebufferBackend::new(
            fb_target.info.width,
            fb_target.info.height,
            fb_target.info.pitch,
            fb_target.addr,
            &mut BACKBUFFER_STORAGE,
        )
    };
    let mut compositor =
        Compositor::init_with_watches(&mut watch_manager, compositor_app_id, backend);
    let mut tick: u64 = 0;
    loop {
        let all_ids = vec![compositor_app_id];

        watch_manager.process_graph(&all_ids);

        for ev in watch_manager.drain_inbox(compositor_app_id) {
            compositor.on_event(&ev);
        }

        // No more tick_apps()
        compositor.tick();
        tick = tick.wrapping_add(1);
        busy_wait();
    }
}

fn discover_framebuffer() -> Option<FramebufferTarget> {
    let info = userland::sys::fb_info()?;
    let addr = userland::sys::fb_map() as *mut u32;
    Some(FramebufferTarget {
        info: FramebufferInfo {
            width: info.width as usize,
            height: info.height as usize,
            pitch: info.pitch as usize,
            bpp: info.bpp as u16,
        },
        addr,
        len_bytes: (info.pitch as usize) * (info.height as usize),
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
