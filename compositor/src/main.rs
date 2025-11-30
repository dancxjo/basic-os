#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;
use alloc::vec;

#[cfg(not(feature = "std"))]
use compositor::{BitmapFramebufferDevice, BitmapRenderer, Compositor, FramebufferTarget};
#[cfg(not(feature = "std"))]
use userland::{println, FramebufferGeometry, WatchManager};

#[cfg(not(feature = "std"))]
const FRAME_INTERVAL_SPINS: usize = 10_000_000;
#[cfg(not(feature = "std"))]
static mut BACKBUFFER_STORAGE: [u32; 8_388_608] = [0; 8_388_608];

#[cfg(not(feature = "std"))]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    userland::init_heap();

    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();

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
        watch_manager.process_graph(&[compositor_app_id]);

        for ev in watch_manager.drain_inbox(compositor_app_id) {
            compositor.on_event(&ev);
        }

        compositor.tick();
        tick = tick.wrapping_add(1);
        busy_wait();
    }
}

#[cfg(not(feature = "std"))]
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

#[cfg(not(feature = "std"))]
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

#[cfg(not(feature = "std"))]
fn busy_wait() {
    for _ in 0..FRAME_INTERVAL_SPINS {
        core::hint::spin_loop();
    }
}

#[cfg(not(feature = "std"))]
#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic inside userland compositor: {info}");
    loop {}
}

#[cfg(feature = "std")]
fn main() {
    use compositor::{Compositor, HostFramebufferDevice, SvgRenderer};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use std::io::Read;
    use tiny_http::{Header, Response, Server};
    use userland::watch::WatchManager;

    userland::ensure_kernel_runtime();

    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();

    let fb_device = HostFramebufferDevice::new(1024, 768);
    let renderer = SvgRenderer::new();
    let compositor = Compositor::<HostFramebufferDevice, SvgRenderer>::init_with_watches(
        &mut watch_manager,
        compositor_app_id,
        fb_device,
        renderer,
    );
    let compositor = Arc::new(Mutex::new(compositor));

    {
        let compositor = compositor.clone();
        thread::spawn(move || {
            let mut watch_manager = watch_manager;
            loop {
                watch_manager.process_graph(&[compositor_app_id]);
                let events = watch_manager.drain_inbox(compositor_app_id);
                {
                    let mut comp = compositor.lock().expect("compositor mutex poisoned");
                    for ev in &events {
                        comp.on_event(ev);
                    }
                    comp.tick();
                }
                thread::sleep(Duration::from_millis(33));
            }
        });
    }

    let server = Server::http("0.0.0.0:8080").expect("failed to bind HTTP server");
    println!("Host compositor: http://127.0.0.1:8080/");

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        if url.starts_with("/input/key") {
             let mut content = String::new();
             if request.as_reader().read_to_string(&mut content).is_ok() {
                 if let Ok(code) = content.trim().parse::<u8>() {
                     userland::host_runtime().push_scancode(code);
                     let _ = request.respond(Response::from_string("OK"));
                     continue;
                 }
             }
             let _ = request.respond(Response::from_string("Bad Request").with_status_code(400));
             continue;
        }

        match url.as_str() {
            "/" | "/frame.svg" => {
                let xml = {
                    let comp = compositor.lock().expect("compositor mutex poisoned");
                    comp.fb_device().artifact().to_string()
                };
                let response = Response::from_string(xml).with_header(
                    "Content-Type: image/svg+xml; charset=utf-8"
                        .parse::<Header>()
                        .unwrap(),
                );
                let _ = request.respond(response);
            }
            _ => {
                let _ = request.respond(Response::from_string("Not found").with_status_code(404));
            }
        }
    }
}
