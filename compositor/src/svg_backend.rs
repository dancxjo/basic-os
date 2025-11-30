#![cfg(feature = "host")]

use crate::{CompositorBackend, CompositorExport, DrawCommand, Rgba, Scene};
use std::fmt::Write;

pub struct SvgBackend {
    width: u32,
    height: u32,
    xml: String,
}

impl SvgBackend {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            xml: String::new(),
        }
    }

    pub fn xml(&self) -> &str {
        &self.xml
    }
}

impl CompositorBackend for SvgBackend {
    fn size(&self) -> (usize, usize) {
        (self.width as usize, self.height as usize)
    }

    fn render(&mut self, scene: &Scene) {
        self.xml.clear();
        writeln!(
            &mut self.xml,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
            w = scene.width,
            h = scene.height,
        )
        .unwrap();

        for cmd in scene.commands() {
            match cmd {
                DrawCommand::Clear { color } => {
                    writeln!(
                        &mut self.xml,
                        r#"<rect x="0" y="0" width="{w}" height="{h}" fill="{fill}"/>"#,
                        w = scene.width,
                        h = scene.height,
                        fill = svg_color(*color),
                    )
                    .unwrap();
                }
                DrawCommand::FillRect { rect, color } => {
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
                DrawCommand::BlitImage {
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
                DrawCommand::DrawText {
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
                DrawCommand::DrawTextBlock { rect, text, color } => {
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
                DrawCommand::DrawCursor {
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

        self.xml.push_str("</svg>");
    }

    fn export(&self) -> Option<CompositorExport<'_>> {
        Some(CompositorExport::Svg {
            xml: &self.xml,
            width: self.width,
            height: self.height,
        })
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
