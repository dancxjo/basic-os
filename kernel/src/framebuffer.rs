use crate::thing::Thingable;
use crate::{bootloader::get_module, thing::Thing};
use alloc::{collections::BTreeMap, vec};
use core::convert::Infallible;
use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
};
use libm::floor;
use limine::request::FramebufferRequest;
use serde::{Deserialize, Serialize};
use thing_macros::Thing;

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
                self.backbuffer[index] = self.encode_color(color);
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

    fn encode_color(&self, color: Rgb565) -> u32 {
        match self.bpp {
            16 => {
                let raw: u16 =
                    ((color.r() as u16) << 11) | ((color.g() as u16) << 5) | (color.b() as u16);
                raw as u32
            }
            24 | 32 => {
                ((color.r() as u32) << 19) | ((color.g() as u32) << 10) | ((color.b() as u32) << 3)
            }
            _ => 0,
        }
    }

    pub fn flush(&mut self) {
        self.fb.copy_from_slice(&self.backbuffer[..self.fb.len()]);
    }

    pub fn clear(&mut self, color: u32) {
        for i in 0..self.pitch_pixels * self.height {
            self.backbuffer[i] = color;
        }
    }

    pub fn draw_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch_pixels + x;
        self.backbuffer[offset] = color;
    }

    pub fn draw_bmp(&mut self, x: usize, y: usize, data: &[u8]) {
        if data.len() < 54 || &data[0..2] != b"BM" {
            return; // Not a BMP
        }

        let offset = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
        let width = u32::from_le_bytes(data[18..22].try_into().unwrap()) as usize;
        let height = u32::from_le_bytes(data[22..26].try_into().unwrap()) as usize;
        let bpp = u16::from_le_bytes(data[28..30].try_into().unwrap());
        let row_size = ((bpp as usize * width + 31) / 32) * 4;

        for row in 0..height {
            let row_start = offset + row * row_size;
            for col in 0..width {
                let pixel_start = row_start + col * 3;
                if pixel_start + 2 >= data.len() {
                    continue;
                }
                let blue = data[pixel_start] as u32;
                let green = data[pixel_start + 1] as u32;
                let red = data[pixel_start + 2] as u32;
                let color = (red << 16) | (green << 8) | blue;
                self.draw_pixel(x + col, y + (height - row - 1), color);
            }
        }
    }

    pub fn draw_text(&mut self, text: &str, x: usize, y: usize, color: u32) {
        let local_color = LocalRgb565::from(color);
        let style = MonoTextStyleBuilder::new()
            .text_color(Rgb565::new(local_color.r, local_color.g, local_color.b))
            .background_color(Rgb565::BLACK)
            .build();

        let pos = Point::new(x as i32, y as i32);
        let text = embedded_graphics::text::Text::new(text, pos, style);
        let _ = text.draw(self);
    }

    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32, alpha: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch_pixels + x;
        let dst = self.backbuffer[offset];

        let sr = ((color >> 16) & 0xFF) as f32;
        let sg = ((color >> 8) & 0xFF) as f32;
        let sb = (color & 0xFF) as f32;

        let dr = ((dst >> 16) & 0xFF) as f32;
        let dg = ((dst >> 8) & 0xFF) as f32;
        let db = (dst & 0xFF) as f32;

        let r = (sr * alpha + dr * (1.0 - alpha)) as u32;
        let g = (sg * alpha + dg * (1.0 - alpha)) as u32;
        let b = (sb * alpha + db * (1.0 - alpha)) as u32;

        self.backbuffer[offset] = (r << 16) | (g << 8) | b;
    }

    pub fn draw_line_aa(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
        fn ipart(x: f32) -> i32 {
            floor(x as f64) as i32
        }
        fn round_f32(x: f32) -> i32 {
            floor(x as f64 + 0.5) as i32
        }
        fn fpart(x: f32) -> f32 {
            x - floor(x as f64) as f32
        }
        fn rfpart(x: f32) -> f32 {
            1.0 - fpart(x)
        }

        let mut x0 = x0;
        let mut y0 = y0;
        let mut x1 = x1;
        let mut y1 = y1;
        let steep = (y1 - y0).abs() > (x1 - x0).abs();

        if steep {
            core::mem::swap(&mut x0, &mut y0);
            core::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            core::mem::swap(&mut x0, &mut x1);
            core::mem::swap(&mut y0, &mut y1);
        }

        let dx = x1 - x0;
        let dy = y1 - y0;
        let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

        // First endpoint
        let xend = round_f32(x0) as f32;
        let yend = y0 + gradient * (xend - x0);
        let xgap = rfpart(x0 + 0.5);
        let xpxl1 = xend as i32;
        let ypxl1 = ipart(yend);

        if steep {
            self.blend_pixel(ypxl1 as usize, xpxl1 as usize, color, rfpart(yend) * xgap);
            self.blend_pixel(
                (ypxl1 + 1) as usize,
                xpxl1 as usize,
                color,
                fpart(yend) * xgap,
            );
        } else {
            self.blend_pixel(xpxl1 as usize, ypxl1 as usize, color, rfpart(yend) * xgap);
            self.blend_pixel(
                xpxl1 as usize,
                (ypxl1 + 1) as usize,
                color,
                fpart(yend) * xgap,
            );
        }

        let mut intery = yend + gradient;

        // Second endpoint
        let xend = round_f32(x1) as f32;
        let yend = y1 + gradient * (xend - x1);
        let xgap = fpart(x1 + 0.5);
        let xpxl2 = xend as i32;
        let ypxl2 = ipart(yend);

        if steep {
            self.blend_pixel(ypxl2 as usize, xpxl2 as usize, color, rfpart(yend) * xgap);
            self.blend_pixel(
                (ypxl2 + 1) as usize,
                xpxl2 as usize,
                color,
                fpart(yend) * xgap,
            );
        } else {
            self.blend_pixel(xpxl2 as usize, ypxl2 as usize, color, rfpart(yend) * xgap);
            self.blend_pixel(
                xpxl2 as usize,
                (ypxl2 + 1) as usize,
                color,
                fpart(yend) * xgap,
            );
        }

        // Main loop
        if steep {
            for x in (xpxl1 + 1)..xpxl2 {
                let y = intery;
                self.blend_pixel(ipart(y) as usize, x as usize, color, rfpart(y));
                self.blend_pixel((ipart(y) + 1) as usize, x as usize, color, fpart(y));
                intery += gradient;
            }
        } else {
            for x in (xpxl1 + 1)..xpxl2 {
                let y = intery;
                self.blend_pixel(x as usize, ipart(y) as usize, color, rfpart(y));
                self.blend_pixel(x as usize, (ipart(y) + 1) as usize, color, fpart(y));
                intery += gradient;
            }
        }
    }
    pub fn draw_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: u32) {
        let mut x0 = x0 as isize;
        let mut y0 = y0 as isize;
        let x1 = x1 as isize;
        let y1 = y1 as isize;

        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.draw_pixel(x0 as usize, y0 as usize, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn draw_polygon(&mut self, points: &[(usize, usize)], color: u32) {
        if points.len() < 2 {
            return;
        }
        for i in 0..points.len() {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            self.draw_line(x0, y0, x1, y1, color);
        }
    }

    pub fn flood_fill(&mut self, x: usize, y: usize, target_color: u32, replacement_color: u32) {
        if target_color == replacement_color {
            return;
        }
        if self.get_pixel(x, y) != target_color {
            return;
        }

        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            if cx >= self.width || cy >= self.height {
                continue;
            }
            if self.get_pixel(cx, cy) != target_color {
                continue;
            }
            self.draw_pixel(cx, cy, replacement_color);
            if cx > 0 {
                stack.push((cx - 1, cy));
            }
            if cy > 0 {
                stack.push((cx, cy - 1));
            }
            stack.push((cx + 1, cy));
            stack.push((cx, cy + 1));
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let offset = y * self.pitch_pixels + x;
        self.backbuffer[offset]
    }
    pub fn draw_circle(&mut self, cx: usize, cy: usize, radius: usize, color: u32) {
        let mut x = radius as isize;
        let mut y = 0isize;
        let mut err = 0isize;

        while x >= y {
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize + y) as usize,
                (cy as isize + x) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - y) as usize,
                (cy as isize + x) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize - y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - y) as usize,
                (cy as isize - x) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize + y) as usize,
                (cy as isize - x) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize - y) as usize,
                color,
            );

            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }

    pub fn draw_ellipse(&mut self, cx: usize, cy: usize, rx: usize, ry: usize, color: u32) {
        let rx2 = (rx * rx) as isize;
        let ry2 = (ry * ry) as isize;
        let mut x = 0isize;
        let mut y = ry as isize;
        let mut px = 0isize;
        let mut py = 2 * rx2 * y;

        let mut p = ry2 - (rx2 * ry as isize) + ((0.25 * rx2 as f64) as isize);
        while px < py {
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize - y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize - y) as usize,
                color,
            );

            x += 1;
            px += 2 * ry2;
            if p < 0 {
                p += ry2 + px;
            } else {
                y -= 1;
                py -= 2 * rx2;
                p += ry2 + px - py;
            }
        }
        let xp = x as f64 + 0.5;
        let yp = y as f64 - 1.0;

        p = (ry2 as f64 * powi(xp, 2) + rx2 as f64 * powi(yp, 2) - rx2 as f64 * ry2 as f64)
            as isize;
        while y >= 0 {
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize + y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize + x) as usize,
                (cy as isize - y) as usize,
                color,
            );
            self.draw_pixel(
                (cx as isize - x) as usize,
                (cy as isize - y) as usize,
                color,
            );

            y -= 1;
            py -= 2 * rx2;
            if p > 0 {
                p += rx2 - py;
            } else {
                x += 1;
                px += 2 * ry2;
                p += rx2 - py + px;
            }
        }
    }

    pub fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        self.draw_line(x, y, x + w - 1, y, color);
        self.draw_line(x, y + h - 1, x + w - 1, y + h - 1, color);
        self.draw_line(x, y, x, y + h - 1, color);
        self.draw_line(x + w - 1, y, x + w - 1, y + h - 1, color);
    }

    pub fn draw_rounded_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        r: usize,
        color: u32,
    ) {
        // Draw straight edges
        self.draw_rect(x + r, y, w - 2 * r, 1, color);
        self.draw_rect(x + r, y + h - 1, w - 2 * r, 1, color);
        self.draw_rect(x, y + r, 1, h - 2 * r, color);
        self.draw_rect(x + w - 1, y + r, 1, h - 2 * r, color);

        // Draw corners using quarter circles
        self.draw_circle_quarter(x + r, y + r, r, 1, color);
        self.draw_circle_quarter(x + w - r - 1, y + r, r, 2, color);
        self.draw_circle_quarter(x + r, y + h - r - 1, r, 3, color);
        self.draw_circle_quarter(x + w - r - 1, y + h - r - 1, r, 4, color);
    }

    fn draw_circle_quarter(&mut self, cx: usize, cy: usize, r: usize, quadrant: u8, color: u32) {
        let mut x = r as isize;
        let mut y = 0isize;
        let mut err = 0isize;

        while x >= y {
            match quadrant {
                1 => self.draw_pixel(
                    (cx as isize - y) as usize,
                    (cy as isize - x) as usize,
                    color,
                ),
                2 => self.draw_pixel(
                    (cx as isize + y) as usize,
                    (cy as isize - x) as usize,
                    color,
                ),
                3 => self.draw_pixel(
                    (cx as isize - y) as usize,
                    (cy as isize + x) as usize,
                    color,
                ),
                4 => self.draw_pixel(
                    (cx as isize + y) as usize,
                    (cy as isize + x) as usize,
                    color,
                ),
                _ => {}
            }
            y += 1;
            if err <= 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err -= 2 * x + 1;
            }
        }
    }
}

pub struct LocalRgb565 {
    r: u8,
    g: u8,
    b: u8,
}

impl LocalRgb565 {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl From<u32> for LocalRgb565 {
    fn from(color: u32) -> Self {
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        LocalRgb565::new(r >> 3, g >> 2, b >> 3)
    }
}

fn powi(x: f64, n: i32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut res = 1.0;
    let mut base = x;
    let mut exp = n.abs();
    while exp > 0 {
        if exp % 2 == 1 {
            res *= base;
        }
        base *= base;
        exp /= 2;
    }
    if n < 0 { 1.0 / res } else { res }
}
