#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;

use compositor::{BitmapFramebufferDevice, BitmapRenderer, Compositor, FramebufferTarget};
use userland::{println, FramebufferGeometry, WatchManager};

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

    let fb_device = unsafe {
        BitmapFramebufferDevice::new(
            fb_target.info.width as usize,
            fb_target.info.height as usize,
            fb_target.info.pitch as usize,
            fb_target.addr,
        )
    };

    let renderer = unsafe {
        BitmapRenderer::new(
            fb_target.info.width as usize,
            fb_target.info.height as usize,
            &mut BACKBUFFER_STORAGE,
        )
    };
    let mut compositor = Compositor::<BitmapFramebufferDevice, BitmapRenderer>::init_with_watches(
        &mut watch_manager,
        compositor_app_id,
        fb_device,
        renderer,
    );
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
        info: FramebufferGeometry {
            width: info.width as u32,
            height: info.height as u32,
            pitch: info.pitch as u32,
            bpp: info.bpp as u16,
        },
        addr,
        len_bytes: (info.pitch as usize) * (info.height as usize),
    })
}

fn fallback_framebuffer() -> FramebufferTarget {
    FramebufferTarget {
        info: FramebufferGeometry {
            width: 1024,
            height: 768,
            pitch: 1024 * 4,
            bpp: 32,
        },
        addr: unsafe { BACKBUFFER_STORAGE.as_mut_ptr() },
        len_bytes: 1024 * 768 * 4,
    }
}

fn busy_wait() {
    for _ in 0..FRAME_INTERVAL_SPINS {
        core::hint::spin_loop();
    }
}

#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic inside userland compositor: {info}");
    loop {}
}
