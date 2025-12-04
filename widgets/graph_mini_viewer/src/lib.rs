#![no_std]

extern crate alloc;

use unifont::get_glyph;
use userland::widget_abi::*;

pub struct GraphMiniViewerWidget;

#[derive(Clone, Debug)]
pub struct State;

impl WidgetAbi for GraphMiniViewerWidget {
    type State = State;

    fn init(_ctx: &WidgetContext) -> Self::State {
        State
    }

    fn draw(_state: &Self::State, fb: &mut [u8], rect: Rect) {
        let stride = 1024; // FIXME

        // Draw background
        for y in rect.y..rect.y + rect.height as i32 {
            for x in rect.x..rect.x + rect.width as i32 {
                if x < 0 || y < 0 {
                    continue;
                }
                let offset = (y as usize * stride + x as usize) * 4;
                if offset + 3 < fb.len() {
                    fb[offset] = 20;
                    fb[offset + 1] = 20;
                    fb[offset + 2] = 20;
                    fb[offset + 3] = 255;
                }
            }
        }

        draw_text(
            fb,
            stride,
            rect.x + 5,
            rect.y + 5,
            "Mini Graph",
            200,
            200,
            200,
        );

        // Draw some fake nodes
        draw_rect(fb, stride, rect.x + 50, rect.y + 50, 20, 20, 255, 0, 0);
        draw_rect(fb, stride, rect.x + 100, rect.y + 80, 20, 20, 0, 255, 0);
        draw_rect(fb, stride, rect.x + 150, rect.y + 40, 20, 20, 0, 0, 255);

        // Draw some fake edges
        draw_line(
            fb,
            stride,
            rect.x + 60,
            rect.y + 60,
            rect.x + 110,
            rect.y + 90,
            255,
            255,
            255,
        );
        draw_line(
            fb,
            stride,
            rect.x + 110,
            rect.y + 90,
            rect.x + 160,
            rect.y + 50,
            255,
            255,
            255,
        );
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

fn draw_rect(fb: &mut [u8], stride: usize, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
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

fn draw_line(
    fb: &mut [u8],
    stride: usize,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    r: u8,
    g: u8,
    b: u8,
) {
    // Bresenham's line algorithm
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && y >= 0 {
            let offset = (y as usize * stride + x as usize) * 4;
            if offset + 3 < fb.len() {
                fb[offset] = b;
                fb[offset + 1] = g;
                fb[offset + 2] = r;
                fb[offset + 3] = 255;
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
