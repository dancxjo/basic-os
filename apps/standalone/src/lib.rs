#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use compositor::{BitmapFramebufferDevice, BitmapRenderer, Compositor, FramebufferTarget};
use userland::app::{create_app, App, AppState, DynApp};
use userland::uuid::Uuid;
use userland::{println, AppContext, FramebufferGeometry, WatchManager};

const FRAME_INTERVAL_SPINS: usize = 1_000_000;
static mut BACKBUFFER_STORAGE: [u32; 8_388_608] = [0; 8_388_608];

pub fn run_desktop(
    runtime: alloc::boxed::Box<dyn thing_abi::ThingRuntime>,
    syscall_handler: unsafe fn(u64, u64, u64, u64, u64) -> u64,
) -> ! {
    unsafe {
        userland::sys::SYSCALL_HANDLER = Some(syscall_handler);
    }
    userland::set_runtime(alloc::boxed::Box::leak(runtime));

    userland::init_heap();

    println!("Single-process desktop: cooperative compositor loop");

    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();

    let fb_target = discover_framebuffer().unwrap_or_else(fallback_framebuffer);
    println!(
        "[INFO] framebuffer ready: {}x{} pitch={} bpp={}",
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
    let renderer = BitmapRenderer::new(
        fb_target.info.width as usize,
        fb_target.info.height as usize,
    );
    let mut compositor = Compositor::<BitmapFramebufferDevice, BitmapRenderer>::init_with_watches(
        &mut watch_manager,
        compositor_app_id,
        fb_device,
        renderer,
    );

    let compositor_id = Uuid::nil();
    let mut apps: Vec<DynApp> = Vec::new();
    println!("[INFO] starting framebuffer_driver in cooperative mode");
    apps.push(create_app::<app_framebuffer_driver::FramebufferDriver>(
        compositor_id,
        &mut watch_manager,
    ));
    println!("[INFO] starting keyboard_driver in cooperative mode");
    apps.push(create_app::<app_keyboard_driver::KeyboardDriver>(
        compositor_id,
        &mut watch_manager,
    ));
    println!("[INFO] starting mouse_driver in cooperative mode");
    let mouse_app_id = watch_manager.register_app();
    let mut mouse_app_state = AppState::new(compositor_id, mouse_app_id);
    let mut mouse_driver = {
        let mut ctx = AppContext {
            watch_manager: &mut watch_manager,
            state: &mut mouse_app_state,
        };
        app_mouse_driver::MouseDriver::init(&mut ctx)
    };

    println!("[INFO] starting widget_host for semantic UI surfaces");
    apps.push(create_app::<widget_host::WidgetHost>(
        compositor_id,
        &mut watch_manager,
    ));
    println!("[INFO] starting text_editor");
    apps.push(create_app::<text_editor::TextEditor>(
        compositor_id,
        &mut watch_manager,
    ));

    let mut participant_ids = alloc::vec![compositor_app_id, mouse_app_id];
    participant_ids.extend(apps.iter().map(|app| app.app_id()));

    let mut tick: u64 = 0;
    loop {
        let events = mouse_driver.poll_events();
        for event in events {
            compositor.on_mouse_event(event.dx as i64, event.dy as i64, event.buttons as u64);
            mouse_driver.emit_mouse_event(event);
        }

        watch_manager.process_graph(&participant_ids);

        for app in apps.iter_mut() {
            app.tick(&mut watch_manager, tick);
        }

        for ev in watch_manager.drain_inbox(compositor_app_id) {
            compositor.on_event(&ev);
        }

        if compositor.is_fb_dirty() {
            if let Some(info) = userland::sys::fb_info() {
                let width = info.width as usize;
                let height = info.height as usize;
                let pitch = info.pitch as usize;
                let addr = info.addr as *mut u32;

                unsafe {
                    compositor
                        .fb_device_mut()
                        .resize(width, height, pitch, addr);
                }
                compositor.renderer_mut().resize(width, height);
                compositor.resize(width, height);
                compositor.clear_fb_dirty();
                println!("Resized compositor to {}x{}", width, height);
            }
        }

        compositor.tick();
        tick = tick.wrapping_add(1);
        cooperative_pause();
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

fn cooperative_pause() {
    for _ in 0..FRAME_INTERVAL_SPINS {
        core::hint::spin_loop();
    }
}
