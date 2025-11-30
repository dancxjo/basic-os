#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;
// use alloc::vec;

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
    let mut tick: u64 = 0;
    loop {
        watch_manager.process_graph(&[compositor_app_id]);

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
    use compositor::{BitmapFramebufferDevice, BitmapRenderer, Compositor, FramebufferTarget};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use userland::watch::WatchManager;
    use userland::FramebufferGeometry;
    // use userland::ThingRuntime;

    userland::ensure_kernel_runtime();

    // Register host apps
    userland::sys::register_host_app("init", init::app_main);
    userland::sys::register_host_app("demo_app", userland::app::run_app::<demo_app::DemoApp>);
    userland::sys::register_host_app(
        "task_list",
        userland::app::run_app::<task_list::TaskListApp>,
    );
    userland::sys::register_host_app(
        "mouse_driver",
        userland::app::run_app::<app_mouse_driver::MouseDriver>,
    );
    userland::sys::register_host_app(
        "keyboard_driver",
        userland::app::run_app::<app_keyboard_driver::KeyboardDriver>,
    );
    userland::sys::register_host_app(
        "framebuffer_driver",
        userland::app::run_app::<app_framebuffer_driver::FramebufferDriver>,
    );
    userland::sys::register_host_app(
        "graph_viewer",
        userland::app::run_app::<graph_viewer::GraphViewerApp>,
    );
    userland::sys::register_host_app(
        "text_editor",
        userland::app::run_app::<text_editor::TextEditor>,
    );
    userland::sys::register_host_app("compositor", || {
        println!("Compositor spawned (ignored)");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });

    // Spawn init process
    userland::sys::spawn("init");

    let mut watch_manager = WatchManager::new();
    let compositor_app_id = watch_manager.register_app();

    // On host, we use the virtual framebuffer from thing_host
    let info = userland::sys::fb_info().expect("failed to get fb info");
    let addr = userland::sys::fb_map() as *mut u32;

    let fb_target = FramebufferTarget {
        info: FramebufferGeometry {
            width: info.width as u32,
            height: info.height as u32,
            pitch: info.pitch as u32,
            bpp: info.bpp as u16,
        },
        addr,
        len_bytes: (info.pitch as usize) * (info.height as usize),
    };

    let fb_device = BitmapFramebufferDevice::new(
        fb_target.info.width as usize,
        fb_target.info.height as usize,
        fb_target.info.pitch as usize,
        fb_target.addr,
    );

    let renderer = BitmapRenderer::new(
        fb_target.info.width as usize,
        fb_target.info.height as usize,
    );

    let compositor = Compositor::<BitmapFramebufferDevice, BitmapRenderer>::init_with_watches(
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
                // Check for resize
                if let Some((w, h, addr)) = userland::host_runtime().check_and_apply_resize() {
                    let mut comp = compositor.lock().expect("compositor mutex poisoned");

                    // Update fb_device
                    comp.fb_device_mut().resize(w, h, w * 4, addr);

                    // Update renderer
                    comp.renderer_mut().resize(w, h);

                    // Update compositor cursor limits
                    comp.resize(w, h);
                }

                watch_manager.process_graph(&[compositor_app_id]);
                let events = watch_manager.drain_inbox(compositor_app_id);
                {
                    let mut comp = compositor.lock().expect("compositor mutex poisoned");
                    for ev in &events {
                        comp.on_event(ev);
                    }
                    comp.tick();
                }
                thread::sleep(Duration::from_millis(16)); // ~60 FPS
            }
        });
    }

    let listener = TcpListener::bind("0.0.0.0:8080").expect("failed to bind HTTP server");
    println!("Host compositor: http://127.0.0.1:8080/");

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            thread::spawn(move || {
                handle_connection(stream);
            });
        }
    }
}

#[cfg(feature = "std")]
fn handle_connection(mut stream: std::net::TcpStream) {
    use std::io::Write;
    use tungstenite::{accept, Message};

    let mut buf = [0u8; 1024];
    // Peek to see if it's a GET request
    let n = match stream.peek(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    if n == 0 {
        return;
    }

    let req_str = String::from_utf8_lossy(&buf[..n]);

    if req_str.starts_with("GET / HTTP") {
        // Serve HTML
        let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>ThingOS Host</title>
    <style>
        body { background: #333; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; overflow: hidden; }
        canvas { background: #000; width: 100%; height: 100%; display: block; }
    </style>
</head>
<body>
<canvas id="screen"></canvas>
<script>
const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');

const ws = new WebSocket('ws://' + location.host + '/ws');
ws.binaryType = 'arraybuffer';

// Default layout uses US AltGr International key positions (set 1 scancodes).
// This maps DOM KeyboardEvent.code strings (physical keys) to PS/2 set 1 scancodes.
const KEYMAP_US_ALTGR_INTL = {
    'Escape': { code: 0x01 },
    'Digit1': { code: 0x02 },
    'Digit2': { code: 0x03 },
    'Digit3': { code: 0x04 },
    'Digit4': { code: 0x05 },
    'Digit5': { code: 0x06 },
    'Digit6': { code: 0x07 },
    'Digit7': { code: 0x08 },
    'Digit8': { code: 0x09 },
    'Digit9': { code: 0x0A },
    'Digit0': { code: 0x0B },
    'Minus': { code: 0x0C },
    'Equal': { code: 0x0D },
    'Backspace': { code: 0x0E },
    'Tab': { code: 0x0F },
    'KeyQ': { code: 0x10 },
    'KeyW': { code: 0x11 },
    'KeyE': { code: 0x12 },
    'KeyR': { code: 0x13 },
    'KeyT': { code: 0x14 },
    'KeyY': { code: 0x15 },
    'KeyU': { code: 0x16 },
    'KeyI': { code: 0x17 },
    'KeyO': { code: 0x18 },
    'KeyP': { code: 0x19 },
    'BracketLeft': { code: 0x1A },
    'BracketRight': { code: 0x1B },
    'Enter': { code: 0x1C },
    'ControlLeft': { code: 0x1D },
    'KeyA': { code: 0x1E },
    'KeyS': { code: 0x1F },
    'KeyD': { code: 0x20 },
    'KeyF': { code: 0x21 },
    'KeyG': { code: 0x22 },
    'KeyH': { code: 0x23 },
    'KeyJ': { code: 0x24 },
    'KeyK': { code: 0x25 },
    'KeyL': { code: 0x26 },
    'Semicolon': { code: 0x27 },
    'Quote': { code: 0x28 },
    'Backquote': { code: 0x29 },
    'ShiftLeft': { code: 0x2A },
    'Backslash': { code: 0x2B },
    'KeyZ': { code: 0x2C },
    'KeyX': { code: 0x2D },
    'KeyC': { code: 0x2E },
    'KeyV': { code: 0x2F },
    'KeyB': { code: 0x30 },
    'KeyN': { code: 0x31 },
    'KeyM': { code: 0x32 },
    'Comma': { code: 0x33 },
    'Period': { code: 0x34 },
    'Slash': { code: 0x35 },
    'ShiftRight': { code: 0x36 },
    'NumpadMultiply': { code: 0x37 },
    'AltLeft': { code: 0x38 },
    'Space': { code: 0x39 },
    'CapsLock': { code: 0x3A },
    'F1': { code: 0x3B },
    'F2': { code: 0x3C },
    'F3': { code: 0x3D },
    'F4': { code: 0x3E },
    'F5': { code: 0x3F },
    'F6': { code: 0x40 },
    'F7': { code: 0x41 },
    'F8': { code: 0x42 },
    'F9': { code: 0x43 },
    'F10': { code: 0x44 },
    'NumLock': { code: 0x45 },
    'ScrollLock': { code: 0x46 },
    'Numpad7': { code: 0x47 },
    'Numpad8': { code: 0x48 },
    'Numpad9': { code: 0x49 },
    'NumpadSubtract': { code: 0x4A },
    'Numpad4': { code: 0x4B },
    'Numpad5': { code: 0x4C },
    'Numpad6': { code: 0x4D },
    'NumpadAdd': { code: 0x4E },
    'Numpad1': { code: 0x4F },
    'Numpad2': { code: 0x50 },
    'Numpad3': { code: 0x51 },
    'Numpad0': { code: 0x52 },
    'NumpadDecimal': { code: 0x53 },
    'F11': { code: 0x57 },
    'F12': { code: 0x58 },
    // Extended keys (E0 prefix)
    'ArrowUp': { code: 0x48, extended: true },
    'ArrowDown': { code: 0x50, extended: true },
    'ArrowLeft': { code: 0x4B, extended: true },
    'ArrowRight': { code: 0x4D, extended: true },
    'Home': { code: 0x47, extended: true },
    'End': { code: 0x4F, extended: true },
    'PageUp': { code: 0x49, extended: true },
    'PageDown': { code: 0x51, extended: true },
    'Insert': { code: 0x52, extended: true },
    'Delete': { code: 0x53, extended: true },
    'NumpadEnter': { code: 0x1C, extended: true },
    'ControlRight': { code: 0x1D, extended: true },
    'AltRight': { code: 0x38, extended: true }, // AltGr
    'MetaLeft': { code: 0x5B, extended: true },
    'MetaRight': { code: 0x5C, extended: true },
    'ContextMenu': { code: 0x5D, extended: true },
    'NumpadDivide': { code: 0x35, extended: true },
};

function sendResize() {
    if (ws.readyState !== WebSocket.OPEN) return;
    const w = window.innerWidth;
    const h = window.innerHeight;
    const packet = new Uint8Array(9);
    packet[0] = 0x52; // 'R'
    new DataView(packet.buffer).setUint32(1, w, true);
    new DataView(packet.buffer).setUint32(5, h, true);
    ws.send(packet);
}

ws.onopen = sendResize;
window.addEventListener('resize', sendResize);

ws.onmessage = function(event) {
    const data = new Uint8Array(event.data);
    if (data.length < 8) return;
    
    const view = new DataView(data.buffer);
    const w = view.getUint32(0, true);
    const h = view.getUint32(4, true);
    
    if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
    }
    
    const pixels = new Uint8ClampedArray(data.buffer, 8);
    if (pixels.length === w * h * 4) {
        const img = new ImageData(pixels, w, h);
        ctx.putImageData(img, 0, 0);
    }
};

function scancodesForEvent(e, released) {
    const entry = KEYMAP_US_ALTGR_INTL[e.code];
    if (!entry) return null;
    const code = entry.code | (released ? 0x80 : 0);
    if (entry.extended) {
        return [0xE0, code];
    }
    return [code];
}

function sendKeyScancodes(scancodes) {
    if (ws.readyState !== WebSocket.OPEN) return;
    const packet = new Uint8Array(1 + scancodes.length);
    packet[0] = 'K'.charCodeAt(0);
    for (let i = 0; i < scancodes.length; i++) {
        packet[i + 1] = scancodes[i];
    }
    ws.send(packet);
}

window.addEventListener('keydown', e => {
    const scancodes = scancodesForEvent(e, false);
    if (scancodes) {
        sendKeyScancodes(scancodes);
        e.preventDefault();
    }
});

window.addEventListener('keyup', e => {
    const scancodes = scancodesForEvent(e, true);
    if (scancodes) {
        sendKeyScancodes(scancodes);
        e.preventDefault();
    }
});

canvas.addEventListener('mousemove', e => {
    if (document.pointerLockElement === canvas) {
        sendMouse(e.movementX, e.movementY, e.buttons);
    }
});

canvas.addEventListener('mousedown', e => {
    if (document.pointerLockElement !== canvas) {
        canvas.requestPointerLock();
    }
    if (document.pointerLockElement === canvas) {
        sendMouse(0, 0, e.buttons);
    }
});

canvas.addEventListener('mouseup', e => {
    if (document.pointerLockElement === canvas) {
        sendMouse(0, 0, e.buttons);
    }
});

function sendMouse(dx, dy, buttons) {
    if (ws.readyState !== WebSocket.OPEN) return;
    
    // PS/2 Mouse Packet Format (3 bytes)
    // Byte 0: Yovfl Xovfl Ysign Xsign 1 M R L
    // Byte 1: X movement
    // Byte 2: Y movement
    
    let flags = 0x08; // Always 1 bit
    if (buttons & 1) flags |= 0x01; // Left
    if (buttons & 2) flags |= 0x02; // Right
    if (buttons & 4) flags |= 0x04; // Middle
    
    if (dx < 0) flags |= 0x10; // X sign
    
    // Clamp movement to -127 to 127
    dx = Math.max(-127, Math.min(127, dx));
    dy = Math.max(-127, Math.min(127, dy));
    
    // Invert Y for PS/2 (up is positive) vs DOM (down is positive)
    let ps2_dy = -dy;
    if (ps2_dy < 0) flags |= 0x20;
    
    const packet = new Uint8Array(3);
    packet[0] = flags;
    packet[1] = dx & 0xFF;
    packet[2] = ps2_dy & 0xFF;
    
    ws.send(packet);
}
</script>
</body>
</html>
"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(response.as_bytes());
    } else if req_str.contains("Upgrade: websocket") {
        // Handle WebSocket
        if let Ok(mut websocket) = accept(stream) {
            let stream = websocket.get_mut();
            // Use blocking mode with a short read timeout to avoid blocking the loop
            if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_millis(1))) {
                println!("Failed to set read timeout: {}", e);
            }

            loop {
                // Send framebuffer
                let bytes = userland::host_runtime().get_framebuffer_bytes();
                let (w, h) = userland::host_runtime().get_size();
                let width = w as u32;
                let height = h as u32;

                let mut msg = Vec::with_capacity(8 + bytes.len());
                msg.extend_from_slice(&width.to_le_bytes());
                msg.extend_from_slice(&height.to_le_bytes());
                msg.extend_from_slice(&bytes);

                if let Err(e) = websocket.send(Message::Binary(msg)) {
                    println!("Write failed: {}", e);
                    break;
                }

                // Read input
                loop {
                    match websocket.read() {
                        Ok(msg) => {
                            if let Message::Text(text) = msg {
                                handle_input(&text);
                            } else if let Message::Binary(data) = msg {
                                handle_binary_input(&data);
                            }
                        }
                        Err(tungstenite::Error::Io(ref e))
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            break;
                        }
                        Err(e) => {
                            println!("Read error: {}", e);
                            return;
                        } // Connection closed or error
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(33)); // ~30 FPS
            }
        }
    }
}

#[cfg(feature = "std")]
fn handle_input(text: &str) {
    if let Some(code_str) = text.strip_prefix("K:") {
        if let Ok(code) = code_str.parse::<u8>() {
            userland::host_runtime().push_scancode(code);
        }
    }
}

#[cfg(feature = "std")]
fn handle_binary_input(data: &[u8]) {
    if !data.is_empty() && data[0] == b'K' {
        for byte in &data[1..] {
            userland::host_runtime().push_scancode(*byte);
        }
    } else if data.len() == 3 {
        userland::host_runtime().push_mouse_packet(data);
    } else if data.len() == 9 && data[0] == b'R' {
        let width = u32::from_le_bytes(data[1..5].try_into().unwrap()) as usize;
        let height = u32::from_le_bytes(data[5..9].try_into().unwrap()) as usize;
        userland::host_runtime().request_resize(width, height);
    }
}
