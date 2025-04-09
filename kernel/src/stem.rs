// Draws the UI by walking the Thing graph, styled after Windows 3.1

use crate::framebuffer::Framebuffer;
use crate::graph::Fact;
use crate::graph::Graph;
use crate::thing::Thing;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_8X13};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::primitives::Triangle;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::{Baseline, Text};

pub fn draw_stem_ui(graph: &Graph, fb: &mut Framebuffer) {
    for fact in &graph.facts {
        let this = &graph.things[fact.this];
        let that = &graph.things[fact.that];

        match (
            this.kind.as_deref().expect("this.kind is None"),
            that.kind.as_deref().expect("that.kind is None"),
        ) {
            ("view-root", "background") => draw_background(fb),
            ("view-root", "window") => draw_window(fb, 64, 64, 320, 200),
            ("window", "label") => draw_label(fb, 80, 80, &String::from_utf8_lossy(&that.data)),
            ("view-root", "pointer") => {
                if that.data.len() >= 2 {
                    draw_pointer(fb, that.data[0] as usize, that.data[1] as usize);
                }
            }
            ("view-root", "log-view") => draw_log_view(fb, 64, 280, 384, 120, &that.data),
            _ => {}
        }
    }
}

fn draw_background(fb: &mut Framebuffer) {
    let bg_color = Rgb565::new(0x00 >> 3, 0x00 >> 2, 0x80 >> 3); // Classic Windows blue
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            fb.draw_iter([Pixel(Point::new(x as i32, y as i32), bg_color)])
                .ok();
        }
    }
}

fn draw_window(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize) {
    let light = Rgb565::new(31, 31, 31); // White (light)
    let dark = Rgb565::new(5, 5, 5); // Dark gray
    let face = Rgb565::new(20, 20, 20); // Window face gray
    let titlebar = Rgb565::new(0x00 >> 3, 0x00 >> 2, 0x80 >> 3); // Classic blue

    // Draw outer border (beveled)
    let outer = Rectangle::new(
        Point::new(x as i32, y as i32),
        Size::new(w as u32, h as u32),
    )
    .into_styled(PrimitiveStyleBuilder::new().fill_color(face).build());
    outer.draw(fb).unwrap();

    // Top + Left bevel (light)
    for dx in 0..w {
        fb.draw_iter([Pixel(Point::new((x + dx) as i32, y as i32), light)])
            .ok();
    }
    for dy in 0..h {
        fb.draw_iter([Pixel(Point::new(x as i32, (y + dy) as i32), light)])
            .ok();
    }

    // Bottom + Right bevel (dark)
    for dx in 0..w {
        fb.draw_iter([Pixel(Point::new((x + dx) as i32, (y + h - 1) as i32), dark)])
            .ok();
    }
    for dy in 0..h {
        fb.draw_iter([Pixel(Point::new((x + w - 1) as i32, (y + dy) as i32), dark)])
            .ok();
    }

    // Titlebar
    let title_rect = Rectangle::new(Point::new(x as i32, y as i32), Size::new(w as u32, 16))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(titlebar).build());
    title_rect.draw(fb).unwrap();

    // Fake title text
    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    Text::with_baseline(
        "Thing OS",
        Point::new((x + 4) as i32, (y + 2) as i32),
        style,
        Baseline::Top,
    )
    .draw(fb)
    .unwrap();

    // Close box (X)
    let box_rect = Rectangle::new(Point::new((x + w - 18) as i32, y as i32), Size::new(14, 14))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(light)
                .stroke_width(1)
                .build(),
        );
    box_rect.draw(fb).unwrap();
    Text::with_baseline(
        "X",
        Point::new((x + w - 14) as i32, (y + 1) as i32),
        style,
        Baseline::Top,
    )
    .draw(fb)
    .unwrap();
}

fn draw_label(fb: &mut Framebuffer, x: usize, y: usize, text: &str) {
    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    let position = Point::new(x as i32, y as i32);
    Text::with_baseline(text, position, style, Baseline::Top)
        .draw(fb)
        .unwrap();
}

fn draw_pointer(fb: &mut Framebuffer, x: usize, y: usize) {
    let style = PrimitiveStyle::with_fill(Rgb565::WHITE);
    let shadow_style = PrimitiveStyle::with_fill(Rgb565::BLACK);

    let triangle = Triangle::new(
        Point::new(x as i32, y as i32),
        Point::new(x as i32 + 6, y as i32 + 12),
        Point::new(x as i32 + 2, y as i32 + 14),
    );

    let shadow = triangle
        .translate(Point::new(1, 1))
        .into_styled(shadow_style);
    let pointer = triangle.into_styled(style);

    shadow.draw(fb).unwrap();
    pointer.draw(fb).unwrap();
}

pub fn update_pointer_position(graph: &mut Graph, fb: &Framebuffer, dx: i8, dy: i8) {
    for thing in &mut graph.things {
        if thing.kind == Some("pointer") {
            let (mut x, mut y) = match thing.data.len() {
                2 => (thing.data[0], thing.data[1]),
                _ => (fb.width() as u8 / 2, fb.height() as u8 / 2),
            };
            x = x.saturating_add_signed(dx);
            y = y.saturating_add_signed(dy);
            thing.data = Box::leak(vec![x, y].into_boxed_slice());
        }
    }
}

fn draw_log_view(fb: &mut Framebuffer, x: usize, y: usize, w: usize, h: usize, data: &[u8]) {
    draw_window(fb, x, y, w, h);

    let style = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    let line_y = y as i32 + 18;
    let max_lines = (h - 18) / 16;

    let lossy = String::from_utf8_lossy(data);
    let lines: Vec<&str> = lossy.lines().collect();

    let start = if lines.len() > max_lines {
        lines.len() - max_lines
    } else {
        0
    };

    for (i, line) in lines[start..].iter().enumerate() {
        let pt = Point::new((x + 6) as i32, line_y + (i * 16) as i32);
        Text::with_baseline(line, pt, style, Baseline::Top)
            .draw(fb)
            .ok();
    }
}
