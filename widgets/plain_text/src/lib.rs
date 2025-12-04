#![no_std]

extern crate alloc;

use alloc::string::String;
use unifont::get_glyph;
use userland::graph::{get_props, GraphPropsGetRequest};
use userland::uuid::Uuid;
use userland::widget_abi::{Rect, WidgetAbi, WidgetContext, WidgetEvent};
use userland::{canon, Value};

pub struct PlainTextWidget;

#[derive(Clone, Debug)]
pub struct State {
    widget_id: Uuid,
    text: String,
}

const CLEAR_COLOR: u32 = 0x0000_0000;
const TEXT_COLOR: (u8, u8, u8) = (0x33, 0x33, 0x33);
const PADDING: i32 = 8;

impl WidgetAbi for PlainTextWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let mut text = String::from("Plain text");
        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::TEXT],
        };
        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                text = t.clone();
            }
        }

        State {
            widget_id: ctx.widget_id,
            text,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let mut text = state.text.clone();
        let req = GraphPropsGetRequest {
            node: state.widget_id,
            keys: alloc::vec![canon::TEXT],
        };
        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                text = t.clone();
            }
        }

        fill_rect(
            fb,
            rect,
            0,
            0,
            rect.width as i32,
            rect.height as i32,
            CLEAR_COLOR,
        );
        draw_multiline_text(fb, rect, PADDING, PADDING, &text, TEXT_COLOR);
    }

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {}

    fn teardown(_state: Self::State) {}
}

fn fill_rect(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let stride = rect.width as usize;
    if stride == 0 {
        return;
    }
    let x0 = rect.x + x;
    let y0 = rect.y + y;
    let x1 = (x0 + w).min(rect.x + rect.width as i32);
    let y1 = (y0 + h).min(rect.y + rect.height as i32);

    for yy in y0.max(0)..y1 {
        for xx in x0.max(0)..x1 {
            let offset = (yy as usize * stride + xx as usize) * 4;
            if offset + 3 < fb.len() {
                fb[offset] = (color >> 16) as u8;
                fb[offset + 1] = (color >> 8) as u8;
                fb[offset + 2] = color as u8;
                fb[offset + 3] = (color >> 24) as u8;
            }
        }
    }
}

fn draw_multiline_text(
    fb: &mut [u8],
    rect: Rect,
    start_x: i32,
    start_y: i32,
    text: &str,
    color: (u8, u8, u8),
) {
    let mut x = start_x;
    let mut y = start_y;
    let stride = rect.width as usize;

    for ch in text.chars() {
        if ch == '\n' {
            x = start_x;
            y += 18;
            continue;
        }
        draw_char(fb, rect, stride, x, y, ch, color);
        x += 8;
    }
}

fn draw_char(
    fb: &mut [u8],
    rect: Rect,
    stride: usize,
    x: i32,
    y: i32,
    ch: char,
    color: (u8, u8, u8),
) {
    if let Some(glyph) = get_glyph(ch) {
        for row in 0..16 {
            for col in 0..8 {
                if glyph.get_pixel(col, row) {
                    let px = rect.x + x + col as i32;
                    let py = rect.y + y + row as i32;
                    if px < 0
                        || py < 0
                        || px >= rect.x + rect.width as i32
                        || py >= rect.y + rect.height as i32
                    {
                        continue;
                    }
                    let offset = (py as usize * stride + px as usize) * 4;
                    if offset + 3 < fb.len() {
                        fb[offset] = color.2;
                        fb[offset + 1] = color.1;
                        fb[offset + 2] = color.0;
                        fb[offset + 3] = 0xFF;
                    }
                }
            }
        }
    }
}
