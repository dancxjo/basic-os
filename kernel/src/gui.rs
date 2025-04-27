use crate::framebuffer::Framebuffer;
use crate::gui_output::GuiOutputBuffer;
use crate::kernel_logger::logger;
use alloc::format;
use alloc::string::String;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::text::Baseline;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10, ascii::FONT_10X20},
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, RoundedRectangle},
    text::Text,
};

pub struct GUI {
    buffer: GuiOutputBuffer,
    message: String,
}

impl GUI {
    pub fn new(framebuffer: &Framebuffer) -> Self {
        GUI {
            buffer: GuiOutputBuffer::from_framebuffer(framebuffer),
            message: String::new(),
        }
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn output(&mut self, framebuffer: &mut Framebuffer) {
        self.buffer.blit_to(framebuffer);
    }

    pub fn draw(&mut self) {
        let width = self.buffer.width as i32;
        let height = self.buffer.height as i32;
        let one_third = width / 3;
        let two_third = width - one_third;

        let background = Rgb565::new(250 >> 3, 250 >> 2, 245 >> 3);
        let box_blue = Rgb565::new(200 >> 3, 230 >> 2, 255 >> 3);
        let box_peach = Rgb565::new(255 >> 3, 230 >> 2, 200 >> 3);
        let header_color = Rgb565::new(180 >> 3, 210 >> 2, 240 >> 3);
        let text_dark = Rgb565::new(30 >> 3, 30 >> 2, 30 >> 3);

        Rectangle::new(Point::zero(), Size::new(width as u32, height as u32))
            .into_styled(PrimitiveStyle::with_fill(background))
            .draw(&mut self.buffer)
            .ok();

        let log_rect = Rectangle::new(
            Point::new(20, 20),
            Size::new(two_third as u32 - 40, height as u32 - 40),
        );
        let repl_rect = Rectangle::new(
            Point::new(two_third + 20, 20),
            Size::new(one_third as u32 - 40, height as u32 - 40),
        );

        RoundedRectangle::with_equal_corners(log_rect, Size::new(12, 12))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(box_blue)
                    .stroke_color(header_color)
                    .stroke_width(1)
                    .build(),
            )
            .draw(&mut self.buffer)
            .ok();

        RoundedRectangle::with_equal_corners(repl_rect, Size::new(12, 12))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(box_peach)
                    .stroke_color(header_color)
                    .stroke_width(1)
                    .build(),
            )
            .draw(&mut self.buffer)
            .ok();

        let text_header = MonoTextStyle::new(&FONT_10X20, text_dark);
        let text_label = MonoTextStyle::new(&FONT_10X20, text_dark);
        let text_log = MonoTextStyle::new(&FONT_6X10, text_dark);

        Text::new("ThingOS v0.1", Point::new(30, 40), text_header)
            .draw(&mut self.buffer)
            .ok();

        Text::new("REPL INPUT", Point::new(two_third + 30, 40), text_header)
            .draw(&mut self.buffer)
            .ok();

        let tick_msg = format!("{}", self.message);
        Text::new(&tick_msg, Point::new(30, 80), text_label)
            .draw(&mut self.buffer)
            .ok();

        Text::new("System Log:", Point::new(30, 120), text_label)
            .draw(&mut self.buffer)
            .ok();

        let mut y = 140;
        for entry in logger().iter() {
            let line = format!("[{}] {}", entry.level, entry.message);
            Text::with_baseline(&line, Point::new(30, y), text_log, Baseline::Top)
                .draw(&mut self.buffer)
                .ok();
            y += 12;
        }

        Text::new(">>", Point::new(two_third + 30, 90), text_label)
            .draw(&mut self.buffer)
            .ok();
    }
}
