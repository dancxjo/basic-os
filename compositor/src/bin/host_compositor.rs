#![cfg(feature = "host")]

use compositor::{Compositor, HostFramebufferDevice, SvgRenderer};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;
use thing_host::HostRuntime;
use tiny_http::{Header, Method, Response, Server};

#[derive(Deserialize, Debug)]
struct ResizeRequest {
    width: u32,
    height: u32,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "kind")]
enum InputEvent {
    #[serde(rename = "mouse_move")]
    MouseMove { x: f32, y: f32, buttons: u8 },
    #[serde(rename = "mouse_down")]
    MouseDown {
        x: f32,
        y: f32,
        buttons: u8,
        button: u8,
    },
    #[serde(rename = "mouse_up")]
    MouseUp {
        x: f32,
        y: f32,
        buttons: u8,
        button: u8,
    },
    #[serde(rename = "key_down")]
    KeyDown { key: String, code: String },
    #[serde(rename = "key_up")]
    KeyUp { key: String, code: String },
}

fn main() {
    let runtime = Box::leak(Box::new(HostRuntime::new()));
    userland::set_runtime(runtime);

    let fb_device = HostFramebufferDevice::new(1024, 768);
    let renderer = SvgRenderer::new();
    let compositor = Arc::new(Mutex::new(Compositor::<HostFramebufferDevice, SvgRenderer>::new(
        fb_device, renderer,
    )));

    {
        let compositor = compositor.clone();
        thread::spawn(move || loop {
            {
                let mut comp = compositor.lock().expect("compositor mutex poisoned");
                comp.tick();
            }
            thread::sleep(std::time::Duration::from_millis(33));
        });
    }

    let server = Server::http("0.0.0.0:8080").expect("failed to bind HTTP server");
    println!("Host compositor: http://127.0.0.1:8080/");

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
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
            path if path.starts_with("/frame.svg") => {
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
            "/framebuffer/resize" if request.method() == &Method::Post => {
                let mut content = String::new();
                request.as_reader().read_to_string(&mut content).unwrap();
                if let Ok(req) = serde_json::from_str::<ResizeRequest>(&content) {
                    let mut comp = compositor.lock().expect("compositor mutex poisoned");
                    comp.fb_device_mut().resize(req.width, req.height);
                    let _ = request.respond(Response::empty(200));
                } else {
                    let _ = request.respond(Response::empty(400));
                }
            }
            "/input" if request.method() == &Method::Post => {
                let mut content = String::new();
                request.as_reader().read_to_string(&mut content).unwrap();
                if let Ok(event) = serde_json::from_str::<InputEvent>(&content) {
                    let mut comp = compositor.lock().expect("compositor mutex poisoned");
                    match event {
                        InputEvent::MouseMove { x, y, buttons } => {
                            comp.set_cursor(x as i32, y as i32, buttons);
                        }
                        InputEvent::MouseDown { x, y, buttons, .. } => {
                            comp.set_cursor(x as i32, y as i32, buttons);
                        }
                        InputEvent::MouseUp { x, y, buttons, .. } => {
                            comp.set_cursor(x as i32, y as i32, buttons);
                        }
                        _ => {}
                    }
                }
                let response = Response::from_string("OK");
                let _ = request.respond(response);
            }
            _ => {
                let response = Response::from_string("Not found").with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}
