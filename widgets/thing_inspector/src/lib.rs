#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use unifont::get_glyph;
use userland::graph::{get_props, GraphPropsGetRequest};
use userland::uuid::Uuid;
use userland::widget_abi::*;
use userland::{canon, Value};

pub struct ThingInspectorWidget;

#[derive(Clone, Debug)]
pub struct State {
    widget_id: Uuid,
    pub thing_id: String,
    pub kind: String,
    pub title: String,
}

impl WidgetAbi for ThingInspectorWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        State {
            widget_id: ctx.widget_id,
            thing_id: "0000-0000-0000-0000".into(),
            kind: "demo_dashboard".into(),
            title: "Dashboard Root".into(),
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        let stride = 1024; // FIXME

        let (thing_id, kind, title) = {
            let req = GraphPropsGetRequest {
                node: state.widget_id,
                keys: alloc::vec![canon::TEXT, canon::KIND, canon::TITLE],
            };
            if let Some(props) = get_props(req) {
                let id = props
                    .get(&canon::TEXT)
                    .and_then(|v| v.as_text())
                    .map(String::from)
                    .unwrap_or_else(|| state.thing_id.clone());
                let kind_val = props
                    .get(&canon::KIND)
                    .and_then(|v| v.as_text())
                    .map(String::from)
                    .unwrap_or_else(|| state.kind.clone());
                let title_val = props
                    .get(&canon::TITLE)
                    .and_then(|v| v.as_text())
                    .map(String::from)
                    .unwrap_or_else(|| state.title.clone());
                (id, kind_val, title_val)
            } else {
                (
                    state.thing_id.clone(),
                    state.kind.clone(),
                    state.title.clone(),
                )
            }
        };

        // Draw background
        for y in rect.y..rect.y + rect.height as i32 {
            for x in rect.x..rect.x + rect.width as i32 {
                if x < 0 || y < 0 {
                    continue;
                }
                let offset = (y as usize * stride + x as usize) * 4;
                if offset + 3 < fb.len() {
                    fb[offset] = 240;
                    fb[offset + 1] = 240;
                    fb[offset + 2] = 240;
                    fb[offset + 3] = 255;
                }
            }
        }

        draw_text(fb, stride, rect.x + 5, rect.y + 5, "Inspector", 0, 0, 0);
        draw_text(
            fb,
            stride,
            rect.x + 5,
            rect.y + 25,
            &format!("ID: {}", thing_id),
            50,
            50,
            50,
        );
        draw_text(
            fb,
            stride,
            rect.x + 5,
            rect.y + 45,
            &format!("Kind: {}", kind),
            50,
            50,
            50,
        );
        draw_text(
            fb,
            stride,
            rect.x + 5,
            rect.y + 65,
            &format!("Title: {}", title),
            50,
            50,
            50,
        );
    }

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {}

    fn teardown(_state: Self::State) {}
}

fn draw_text(fb: &mut [u8], stride: usize, x: i32, y: i32, text: &str, r: u8, g: u8, b: u8) {
    let mut cx = x;
    for c in text.chars() {
        if let Some(glyph) = get_glyph(c) {
            let glyph_width = glyph.get_width();
            for row in 0..16 {
                for col in 0..glyph_width {
                    if glyph.get_pixel(col, row) {
                        let px = cx + col as i32;
                        let py = y + row as i32;
                        if px < 0 || py < 0 {
                            continue;
                        }
                        let offset = (py as usize * stride + px as usize) * 4;
                        if offset + 3 < fb.len() {
                            fb[offset] = b;
                            fb[offset + 1] = g;
                            fb[offset + 2] = r;
                            fb[offset + 3] = 255;
                        }
                    }
                }
            }
            cx += glyph_width as i32;
        }
    }
}
