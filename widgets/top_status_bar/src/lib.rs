#![no_std]

extern crate alloc;

use alloc::string::String;
use unifont::get_glyph;
use userland::widget_abi::*;

pub struct TopStatusBarWidget;

pub struct State {
    title: String,
}

impl WidgetAbi for TopStatusBarWidget {
    type State = State;

    fn init(_ctx: &WidgetContext) -> Self::State {
        State {
            title: String::from("ThingOS Demo Dashboard"),
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        let stride = 1024; // FIXME: pass stride in context or rect

        // Draw background
        for y in rect.y..rect.y + rect.height as i32 {
            for x in rect.x..rect.x + rect.width as i32 {
                if x < 0 || y < 0 {
                    continue;
                }
                let offset = (y as usize * stride + x as usize) * 4;
                if offset + 3 < fb.len() {
                    fb[offset] = 34; // 0x22
                    fb[offset + 1] = 34;
                    fb[offset + 2] = 34;
                    fb[offset + 3] = 255;
                }
            }
        }

        // Draw Title
        draw_text(
            fb,
            stride,
            rect.x + 10,
            rect.y + 8,
            &state.title,
            255,
            255,
            255,
        );

        // Draw fake time on the right
        let time_str = "12:00 PM";
        let time_width = time_str.len() as i32 * 8;
        draw_text(
            fb,
            stride,
            rect.x + rect.width as i32 - time_width - 10,
            rect.y + 8,
            time_str,
            200,
            200,
            200,
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
