use crate::bootloader::get_module;
use crate::framebuffer::Framebuffer;
use crate::graph::{Graph, ThingData};
use alloc::string::String;
use alloc::vec::Vec;
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_8X13};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle};
use embedded_graphics::text::{Baseline, Text};
use tinybmp::Bmp;

pub fn draw_stem_ui(graph: &Graph, fb: &mut Framebuffer) {
    for fact in &graph.facts {
        let this = &graph.things[fact.this];
        let that = &graph.things[fact.that];

        match (this.kind, that.kind) {
            ("view-root", "background") => draw_background(fb),
            ("view-root", "window") => draw_window(fb, 64, 64, 320, 200),
            ("window", "label") => {
                if let Some(bytes) = that.data.as_bytes() {
                    let text = String::from_utf8_lossy(bytes);
                    draw_label(fb, 80, 80, &text);
                }
            }
            ("view-root", "pointer") => {
                if let Some(data) = that.data.as_bytes() {
                    if data.len() >= 2 {
                        draw_pointer(fb, data[0] as usize, data[1] as usize);
                    }
                }
            }
            ("view-root", "log-view") => {
                if let Some(data) = that.data.as_bytes() {
                    draw_log_view(fb, 64, 280, 1024, 400, data);
                }
            }
            _ => {}
        }
    }
}

fn draw_background(fb: &mut Framebuffer) {
    let bg_color = Rgb565::new(0, 0, 16);
    let bmp_data = get_module("clouds.bmp").expect("Failed to load clouds.bmp");
    let bmp = Bmp::from_slice(bmp_data).unwrap();
    let bmp_width = bmp.size().width as i32;
    let bmp_height = bmp.size().height as i32;

    for y in (0..fb.height()).step_by(bmp_height as usize) {
        for x in (0..fb.width()).step_by(bmp_width as usize) {
            Image::new(&bmp, Point::new(x as i32, y as i32))
                .draw(fb)
                .unwrap();
        }
    }
}

fn draw_window(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize) {
    let face = Rgb565::new(31, 63, 31); // #C0C0C0
    let light = Rgb565::WHITE; // #FFFFFF
    let shadow = Rgb565::new(16, 32, 16); // #808080
    // let titlebar = Rgb565::new(0, 0, 16); // #000080
    let titlebar = Rgb565::new(6, 30, 24); // #307BC7

    // Outer border
    Rectangle::new(
        Point::new(x as i32, y as i32),
        Size::new(w as u32, h as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(shadow))
    .draw(fb)
    .unwrap();

    // Inner highlight border
    Rectangle::new(
        Point::new((x + 1) as i32, (y + 1) as i32),
        Size::new((w - 2) as u32, (h - 2) as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(light))
    .draw(fb)
    .unwrap();

    // Face panel
    Rectangle::new(
        Point::new((x + 2) as i32, (y + 2 + 16) as i32),
        Size::new((w - 4) as u32, (h - 4 - 16) as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(face))
    .draw(fb)
    .unwrap();

    // Title bar
    Rectangle::new(
        Point::new((x + 2) as i32, (y + 2) as i32),
        Size::new((w - 4) as u32, 16),
    )
    .into_styled(PrimitiveStyle::with_fill(titlebar))
    .draw(fb)
    .unwrap();

    // Title text
    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    Text::with_baseline(
        "Thing OS",
        Point::new((x + 5) as i32, (y + 4) as i32),
        style,
        Baseline::Top,
    )
    .draw(fb)
    .unwrap();

    // Close button
    Rectangle::new(
        Point::new((x + w - 18) as i32, (y + 3) as i32),
        Size::new(14, 12),
    )
    .into_styled(
        PrimitiveStyleBuilder::new()
            .stroke_color(Rgb565::WHITE)
            .stroke_width(1)
            .build(),
    )
    .draw(fb)
    .unwrap();

    // Text::with_baseline(
    //     "x",
    //     Point::new((x + w - 14) as i32, (y + 4) as i32),
    //     style,
    //     Baseline::Top,
    // )
    // .draw(fb)
    // .unwrap();
    fb.draw_char(x + w - 16, y, '×', 0xffffff);
}

fn draw_label(fb: &mut Framebuffer, x: usize, y: usize, text: &str) {
    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::BLACK);
    Text::with_baseline(text, Point::new(x as i32, y as i32), style, Baseline::Top)
        .draw(fb)
        .unwrap();
}

fn draw_pointer(fb: &mut Framebuffer, x: usize, y: usize) {
    let white = PrimitiveStyle::with_fill(Rgb565::WHITE);
    let shadow = PrimitiveStyle::with_fill(Rgb565::BLACK);

    let triangle = Triangle::new(
        Point::new(x as i32, y as i32),
        Point::new(x as i32 + 7, y as i32 + 14),
        Point::new(x as i32 + 3, y as i32 + 16),
    );

    triangle
        .translate(Point::new(1, 1))
        .into_styled(shadow)
        .draw(fb)
        .unwrap();
    triangle.into_styled(white).draw(fb).unwrap();
}

fn draw_log_view(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, data: &[u8]) {
    draw_window(fb, x, y, w, h);

    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::BLACK);
    let line_y = y as i32 + 20;
    let max_lines = (h - 20) / 16;

    let binding = String::from_utf8_lossy(data);
    let lines: Vec<&str> = binding.lines().collect();
    let start = lines.len().saturating_sub(max_lines);

    for (i, line) in lines[start..].iter().enumerate() {
        let pt = Point::new((x + 6) as i32, line_y + (i * 16) as i32);
        Text::with_baseline(line, pt, style, Baseline::Top)
            .draw(fb)
            .ok();
    }
}

pub fn update_pointer_position(graph: &mut Graph, fb: &Framebuffer, dx: i8, dy: i8) {
    for thing in &mut graph.things {
        if thing.kind == "pointer" {
            let (mut x, mut y) = match thing.data.as_bytes() {
                Some(data) if data.len() >= 2 => (data[0], data[1]),
                _ => (fb.width() as u8 / 2, fb.height() as u8 / 2),
            };
            x = x.saturating_add_signed(dx);
            y = y.saturating_add_signed(dy);
            thing.data = ThingData::Heap(alloc::vec![x, y].into_boxed_slice());
        }
    }
}
