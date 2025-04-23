// Framebuffer module using embedded-graphics directly without custom draw_* methods

use alloc::format;
use core::convert::Infallible;
use embedded_graphics::{
    mono_font::iso_8859_1::FONT_6X10,
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Primitive, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::{Baseline, Text},
};
use limine::request::FramebufferRequest;
use serde::Serialize;
use thing_macros::Kind;

const MAX_WIDTH: usize = 3840;
const MAX_HEIGHT: usize = 2160;

#[unsafe(link_section = ".bss.uninit")]
#[unsafe(no_mangle)]
pub static mut BACKBUFFER: [u32; MAX_WIDTH * MAX_HEIGHT] = [0; MAX_WIDTH * MAX_HEIGHT];

#[derive(Debug)]
pub struct Framebuffer {
    fb: &'static mut [u32],
    backbuffer: &'static mut [u32],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pitch_pixels: usize,
    pub bpp: u16,
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let x = coord.x as usize;
            let y = coord.y as usize;
            if x < self.width && y < self.height {
                let index = y * self.pitch_pixels + x;
                self.backbuffer[index] = self.encode_color_rgb565(color);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl Framebuffer {
    pub fn new() -> Option<Self> {
        static FB_REQUEST: FramebufferRequest = FramebufferRequest::new();
        let response = FB_REQUEST.get_response()?;
        let fb_info = response.framebuffers().next()?;

        let width = fb_info.width() as usize;
        let height = fb_info.height() as usize;
        let pitch = fb_info.pitch() as usize;
        let pitch_pixels = pitch / 4;
        let len = pitch_pixels * height;
        let fb_ptr = fb_info.addr() as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };
        let backbuffer = unsafe { &mut BACKBUFFER[..len] };

        Some(Self {
            fb: fb_slice,
            backbuffer,
            width,
            height,
            pitch,
            pitch_pixels,
            bpp: fb_info.bpp(),
        })
    }

    fn encode_color_rgb565(&self, color: Rgb565) -> u32 {
        match self.bpp {
            16 => {
                let raw: u16 =
                    ((color.r() as u16) << 11) | ((color.g() as u16) << 5) | (color.b() as u16);
                raw as u32
            }
            24 | 32 => {
                // Scale up the 5/6/5-bit color to 8-bit
                let r = (color.r() as u32) << 3;
                let g = (color.g() as u32) << 2;
                let b = (color.b() as u32) << 3;
                (r << 16) | (g << 8) | b
            }
            _ => 0,
        }
    }

    pub fn flush(&mut self) {
        self.fb.copy_from_slice(&self.backbuffer[..self.fb.len()]);
    }

    pub fn clear(&mut self, color: Rgb565) {
        let encoded = self.encode_color_rgb565(color);
        for px in self.backbuffer.iter_mut() {
            *px = encoded;
        }
    }
}

use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_10X20};

use crate::kernel_logger::logger;

// TODO: Just a placeholder for now
pub fn draw_kernel_ui(framebuffer: &mut crate::framebuffer::Framebuffer, tick_count: u64) {
    let width = framebuffer.width as i32;
    let height = framebuffer.height as i32;

    let one_third = width / 3;
    let two_third = width - one_third;

    // Color palette
    let background = Rgb565::new(250 >> 3, 250 >> 2, 245 >> 3); // soft parchment background
    let box_blue = Rgb565::new(200 >> 3, 230 >> 2, 255 >> 3); // pale blue
    let box_peach = Rgb565::new(255 >> 3, 230 >> 2, 200 >> 3); // warm peach
    let header_color = Rgb565::new(180 >> 3, 210 >> 2, 240 >> 3); // muted cyan
    let text_dark = Rgb565::new(30 >> 3, 30 >> 2, 30 >> 3); // deep gray

    // Fill entire background
    Rectangle::new(Point::zero(), Size::new(width as u32, height as u32))
        .into_styled(PrimitiveStyle::with_fill(background))
        .draw(framebuffer)
        .ok();

    // Rounded panels
    let log_rect = Rectangle::new(
        Point::new(20, 20),
        Size::new(two_third as u32 - 40, height as u32 - 40),
    );
    let repl_rect = Rectangle::new(
        Point::new(two_third + 20, 20),
        Size::new(one_third as u32 - 40, height as u32 - 40),
    );

    // Draw filled rounded boxes
    RoundedRectangle::with_equal_corners(log_rect, Size::new(12, 12))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(box_blue)
                .stroke_color(header_color)
                .stroke_width(1)
                .build(),
        )
        .draw(framebuffer)
        .ok();

    RoundedRectangle::with_equal_corners(repl_rect, Size::new(12, 12))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(box_peach)
                .stroke_color(header_color)
                .stroke_width(1)
                .build(),
        )
        .draw(framebuffer)
        .ok();

    // Headers
    let header_height = 30;
    Rectangle::new(
        log_rect.top_left,
        Size::new(log_rect.size.width, header_height),
    )
    .into_styled(PrimitiveStyle::with_fill(header_color))
    .draw(framebuffer)
    .ok();

    Rectangle::new(
        repl_rect.top_left,
        Size::new(repl_rect.size.width, header_height),
    )
    .into_styled(PrimitiveStyle::with_fill(header_color))
    .draw(framebuffer)
    .ok();

    // Fonts
    let text_header = MonoTextStyle::new(&FONT_10X20, text_dark);
    let text_label = MonoTextStyle::new(&FONT_10X20, text_dark);
    let text_log = MonoTextStyle::new(&FONT_6X10, text_dark);

    // Titles
    Text::new("ThingOS v0.1", Point::new(30, 40), text_header)
        .draw(framebuffer)
        .ok();

    Text::new("REPL INPUT", Point::new(two_third + 30, 40), text_header)
        .draw(framebuffer)
        .ok();

    // Tick counter & system log
    let tick_msg = format!("Tick: {}", tick_count);
    Text::new(&tick_msg, Point::new(30, 80), text_label)
        .draw(framebuffer)
        .ok();

    Text::new("System Log:", Point::new(30, 120), text_label)
        .draw(framebuffer)
        .ok();

    let mut y = 140;
    for entry in logger().iter() {
        let line = format!("[{}] {}", entry.level, entry.message);
        Text::with_baseline(&line, Point::new(30, y), text_log, Baseline::Top)
            .draw(framebuffer)
            .ok();
        y += 12;
    }

    // REPL input prompt
    Text::new(">>", Point::new(two_third + 30, 90), text_label)
        .draw(framebuffer)
        .ok();

    framebuffer.flush();
}
