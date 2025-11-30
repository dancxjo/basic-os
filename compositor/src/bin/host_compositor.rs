#![cfg(feature = "host")]

use compositor::{Compositor, CompositorBackend, CompositorExport, SvgBackend};
use std::sync::{Arc, Mutex};
use std::thread;
use thing_host::HostRuntime;
use tiny_http::{Header, Response, Server};

fn main() {
    let runtime = Box::leak(Box::new(HostRuntime::new()));
    userland::set_runtime(runtime);

    let backend = SvgBackend::new(1024, 768);
    let compositor = Arc::new(Mutex::new(Compositor::new(backend)));

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

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        match url.as_str() {
            "/" => {
                let html = r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>ThingOS Host Compositor</title></head>
  <body style="margin:0; background:#111; color:#eee; font-family:sans-serif;">
    <h1 style="font-size:14px; margin:4px;">ThingOS Host Compositor</h1>
    <object id="view" type="image/svg+xml" data="/frame.svg" style="width:100%; height:90vh; border:1px solid #444;"></object>
    <script>
      setInterval(function() {
        var obj = document.getElementById('view');
        obj.data = '/frame.svg?ts=' + Date.now();
      }, 250);
    </script>
  </body>
</html>
"#;
                let response = Response::from_string(html).with_header(
                    "Content-Type: text/html; charset=utf-8"
                        .parse::<Header>()
                        .unwrap(),
                );
                let _ = request.respond(response);
            }
            path if path.starts_with("/frame.svg") => {
                let xml = {
                    let comp = compositor.lock().expect("compositor mutex poisoned");
                    match comp.backend().export() {
                        Some(CompositorExport::Svg { xml, .. }) => xml.to_string(),
                        _ => "<svg/>".to_string(),
                    }
                };
                let response = Response::from_string(xml).with_header(
                    "Content-Type: image/svg+xml; charset=utf-8"
                        .parse::<Header>()
                        .unwrap(),
                );
                let _ = request.respond(response);
            }
            _ => {
                let response = Response::from_string("Not found").with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}
