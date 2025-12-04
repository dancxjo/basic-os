#![no_std]

extern crate alloc;

use alloc::string::String;
use unifont::get_glyph;
use userland::graph::{get_props, GraphPropsGetRequest};
use userland::uuid::Uuid;
use userland::widget_abi::*;
use userland::{canon, Value};

pub struct StatusWidget;

#[derive(Clone, Debug)]
pub struct State {
    widget_id: Uuid,
    pub status_text: String,
}

impl WidgetAbi for StatusWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        State {
            widget_id: ctx.widget_id,
            status_text: "Graph: Connected | Input: OK".into(),
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        let stride = 1024; // FIXME

        let status = {
            let req = GraphPropsGetRequest {
                node: state.widget_id,
                keys: alloc::vec![canon::TEXT],
            };
            if let Some(props) = get_props(req) {
                if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                    t.clone()
                } else {
                    state.status_text.clone()
                }
            } else {
                state.status_text.clone()
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
                    fb[offset] = 30;
                    fb[offset + 1] = 30;
                    fb[offset + 2] = 30;
                    fb[offset + 3] = 255;
                }
            }
        }

        draw_text(fb, stride, rect.x + 5, rect.y + 5, &status, 0, 255, 0);
    }

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {}

    fn teardown(_state: Self::State) {}
}

fn draw_text(fb: &mut [u8], stride: usize, x: i32, y: i32, text: &str, r: u8, g: u8, b: u8) {
    let mut cx = x;
    for c in text.chars() {
        if let Some(glyph) = get_glyph(c) {
            for row in 0..16 {
                for col in 0..8 {
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
            cx += 8;
        }
    }
}
