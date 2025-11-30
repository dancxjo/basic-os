#![cfg(feature = "host")]

use crate::{FramebufferDevice, FramebufferGeometry, RendererBackend, Rgba, Scene, SceneItem};
use std::fmt::Write;

pub struct SvgRenderer {
    xml: String,
}

pub struct HostFramebufferDevice {
    width: u32,
    height: u32,
    artifact: String,
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self { xml: String::new() }
    }
}

impl HostFramebufferDevice {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            artifact: String::new(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }
}

impl RendererBackend for SvgRenderer {
    type Output<'a> = String;
    fn render<'a>(&'a mut self, scene: &Scene) -> Self::Output<'a> {
        self.xml.clear();
        writeln!(
            &mut self.xml,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" onload="init(evt)" tabindex="0" style="outline: none">"#,
            w = scene.width,
            h = scene.height,
        )
        .unwrap();

        writeln!(
            &mut self.xml,
            r#"<script type="application/ecmascript"><![CDATA[
    function init(evt) {{
      const svg = evt.target;
      const doc = svg.ownerDocument;
      const win = doc.defaultView || doc.parentWindow;

      svg.focus();
      setInterval(refresh, 250);

      function sendResize() {{
        const w = win.innerWidth;
        const h = win.innerHeight;
        fetch('/framebuffer/resize', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ width: w, height: h }})
        }});
      }}

      win.addEventListener('resize', sendResize);
      sendResize();

      function toSvgCoords(e) {{
        const pt = svg.createSVGPoint();
        pt.x = e.clientX;
        pt.y = e.clientY;
        const svgPt = pt.matrixTransform(svg.getScreenCTM().inverse());
        return {{ x: svgPt.x, y: svgPt.y }};
      }}

      doc.addEventListener('mousemove', function(e) {{
        const coords = toSvgCoords(e);
        sendInput({{
          kind: 'mouse_move',
          x: coords.x,
          y: coords.y,
          buttons: e.buttons
        }});
      }});

      doc.addEventListener('mousedown', function(e) {{
        const coords = toSvgCoords(e);
        sendInput({{
          kind: 'mouse_down',
          x: coords.x,
          y: coords.y,
          buttons: e.buttons,
          button: e.button
        }});
      }});

      doc.addEventListener('mouseup', function(e) {{
        const coords = toSvgCoords(e);
        sendInput({{
          kind: 'mouse_up',
          x: coords.x,
          y: coords.y,
          buttons: e.buttons,
          button: e.button
        }});
      }});

      doc.addEventListener('keydown', function(e) {{
        e.preventDefault();
        sendInput({{
          kind: 'key_down',
          key: e.key,
          code: e.code
        }});
      }});

      doc.addEventListener('keyup', function(e) {{
        e.preventDefault();
        sendInput({{
          kind: 'key_up',
          key: e.key,
          code: e.code
        }});
      }});
    }}

    function refresh() {{
      fetch('/frame.svg?ts=' + Date.now())
        .then(r => r.text())
        .then(text => {{
          const parser = new DOMParser();
          const doc = parser.parseFromString(text, "image/svg+xml");
          const newScene = doc.getElementById('scene');
          const oldScene = document.getElementById('scene');
          if (newScene && oldScene) {{
            oldScene.replaceWith(newScene);
          }}
        }})
        .catch(e => console.error(e));
    }}

    function sendInput(event) {{
      fetch('/input', {{
        method: 'POST',
        headers: {{'Content-Type': 'application/json'}},
        body: JSON.stringify(event)
      }}).catch(_ => {{}});
    }}
  ]]></script>"#
        )
        .unwrap();

        writeln!(&mut self.xml, r#"<g id="scene">"#).unwrap();

        for item in scene.items() {
            match item {
                SceneItem::Clear { color } => {
                    writeln!(
                        &mut self.xml,
                        r#"<rect x="0" y="0" width="{w}" height="{h}" fill="{fill}"/>"#,
                        w = scene.width,
                        h = scene.height,
                        fill = svg_color(*color),
                    )
                    .unwrap();
                }
                SceneItem::FillRect { rect, color } => {
                    writeln!(
                        &mut self.xml,
                        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="2" ry="2" fill="{fill}"/>"#,
                        x = rect.x,
                        y = rect.y,
                        w = rect.width,
                        h = rect.height,
                        fill = svg_color(*color),
                    )
                    .unwrap();
                }
                SceneItem::BlitImage {
                    rect,
                    image,
                    repeat,
                } => {
                    let _ = (image, repeat);
                    writeln!(
                        &mut self.xml,
                        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" fill-opacity="0.3"/>"#,
                        x = rect.x,
                        y = rect.y,
                        w = rect.width,
                        h = rect.height,
                        fill = "#888888",
                    )
                    .unwrap();
                }
                SceneItem::DrawText {
                    origin,
                    text,
                    color,
                    max_width: _,
                } => {
                    let (x, y) = *origin;
                    writeln!(
                        &mut self.xml,
                        r#"<text x="{x}" y="{y}" font-family="monospace" font-size="12" fill="{fill}">{text}</text>"#,
                        x = x,
                        y = y + 10,
                        fill = svg_color(*color),
                        text = escape_xml(text),
                    )
                    .unwrap();
                }
                SceneItem::DrawTextBlock { rect, text, color } => {
                    writeln!(
                        &mut self.xml,
                        r#"<text x="{x}" y="{y}" font-family="monospace" font-size="10" fill="{fill}">{text}</text>"#,
                        x = rect.x,
                        y = rect.y + 12,
                        fill = svg_color(*color),
                        text = escape_xml(text),
                    )
                    .unwrap();
                }
                SceneItem::DrawCursor {
                    origin,
                    primary,
                    shadow: _,
                    pressed,
                } => {
                    let (x, y) = *origin;
                    let size = 16;
                    let opacity = if *pressed { 1.0 } else { 0.8 };
                    writeln!(
                        &mut self.xml,
                        r#"<polygon points="{points}" fill="{fill}" fill-opacity="{opacity}"/>"#,
                        points = format!(
                            "{x},{y} {x},{y2} {x2},{y}",
                            x = x,
                            y = y,
                            y2 = y + size,
                            x2 = x + size,
                        ),
                        fill = svg_color(*primary),
                        opacity = opacity,
                    )
                    .unwrap();
                }
            }
        }

        writeln!(&mut self.xml, r#"</g>"#).unwrap();
        writeln!(&mut self.xml, "</svg>").unwrap();
        self.xml.clone()
    }
}

impl FramebufferDevice<String> for HostFramebufferDevice {
    fn geometry(&self) -> FramebufferGeometry {
        FramebufferGeometry {
            width: self.width,
            height: self.height,
            pitch: self.width * 4,
            bpp: 32,
        }
    }

    fn present(&mut self, frame: String) {
        self.artifact = frame;
    }
}

fn svg_color(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
