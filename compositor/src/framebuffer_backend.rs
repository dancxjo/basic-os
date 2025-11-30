use crate::{
    clamp_i32, Bitmap, FrameInfo, FramebufferDevice, FramebufferGeometry, Rect, RendererBackend,
    Rgba, Scene, SceneItem, CLEAR_COLOR, FONT_HEIGHT,
};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::min;
use unifont::get_glyph;

pub struct BitmapRenderer {
    width: usize,
    height: usize,
    storage: Vec<u32>,
}

pub struct BitmapFramebufferDevice {
    width: usize,
    height: usize,
    pitch: usize,
    addr: *mut u32,
}

unsafe impl Send for BitmapFramebufferDevice {}
unsafe impl Sync for BitmapFramebufferDevice {}

impl BitmapRenderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            storage: vec![0; width * height],
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.storage.resize(width * height, 0);
    }

    fn clear(&mut self, color: Rgba) {
        self.storage.fill(color.to_u32());
    }
}

impl RendererBackend for BitmapRenderer {
    type Output<'b>
        = &'b [u32]
    where
        Self: 'b;
    fn render<'b>(&'b mut self, scene: &Scene) -> Self::Output<'b> {
        self.clear(CLEAR_COLOR);
        for item in scene.items() {
            match item {
                SceneItem::Clear { color } => {
                    self.clear(*color);
                }
                SceneItem::FillRect { rect, color } => {
                    raster_fill_rect(self, rect, *color);
                }
                SceneItem::BlitImage {
                    rect,
                    image,
                    repeat,
                    offset,
                } => {
                    raster_blit_image(self, rect, image, *repeat, *offset);
                }
                SceneItem::DrawText {
                    origin,
                    text,
                    color,
                    max_width,
                } => {
                    raster_draw_text(self, *origin, text, *color, *max_width);
                }
                SceneItem::DrawTextBlock { rect, text, color } => {
                    raster_draw_text_block(self, rect, text, *color);
                }
                SceneItem::DrawCursor {
                    origin,
                    primary,
                    shadow,
                    pressed,
                } => {
                    raster_draw_cursor(self, *origin, *primary, *shadow, *pressed);
                }
            }
        }
        &self.storage
    }
}

impl BitmapFramebufferDevice {
    pub fn new(
        raw_width: usize,
        raw_height: usize,
        raw_pitch_bytes: usize,
        addr: *mut u32,
    ) -> Self {
        let fb_info = crate::sanitize_fb_info(crate::FramebufferGeometry {
            width: raw_width as u32,
            height: raw_height as u32,
            pitch: raw_pitch_bytes as u32,
            bpp: 32,
        });

        Self {
            width: fb_info.width as usize,
            height: fb_info.height as usize,
            pitch: fb_info.pitch as usize,
            addr,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize, pitch: usize, addr: *mut u32) {
        self.width = width;
        self.height = height;
        self.pitch = pitch;
        self.addr = addr;
    }
}

impl<'a> FramebufferDevice<&'a [u32]> for BitmapFramebufferDevice {
    fn geometry(&self) -> FramebufferGeometry {
        FramebufferGeometry {
            width: self.width as u32,
            height: self.height as u32,
            pitch: self.pitch as u32,
            bpp: 32,
        }
    }

    fn present(&mut self, frame: &'a [u32]) {
        if self.addr.is_null() || self.pitch == 0 || self.height == 0 {
            return;
        }

        let stride_u32 = self.pitch / 4;
        let copy_width = min(self.width, stride_u32);
        let copy_height = min(self.height, frame.len() / self.width);

        for y in 0..copy_height {
            let src_start = y * self.width;
            let src_end = src_start + copy_width;
            let dst_start = y * stride_u32;

            if src_end <= frame.len() {
                let src_row = &frame[src_start..src_end];
                unsafe {
                    let dst_ptr = self.addr.add(dst_start);
                    let dst_row = core::slice::from_raw_parts_mut(dst_ptr, copy_width);
                    dst_row.copy_from_slice(src_row);
                }
            }
        }
    }

    fn frame_info(&self) -> Option<FrameInfo> {
        Some(FrameInfo {
            addr: self.addr as u64,
            width: self.width as u64,
            height: self.height as u64,
            pitch: self.pitch as u64,
            bpp: 32,
        })
    }
}

fn raster_fill_rect(backend: &mut BitmapRenderer, rect: &Rect, color: Rgba) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    let color_u32 = color.to_u32();

    if color.a == 0xFF {
        for yy in y0..y1 {
            let row = yy * backend.width;
            for xx in x0..x1 {
                backend.storage[row + xx] = color_u32;
            }
        }
    } else {
        for yy in y0..y1 {
            let row = yy * backend.width;
            for xx in x0..x1 {
                let bg = backend.storage[row + xx];
                backend.storage[row + xx] = userland::graphics::blend(color_u32, bg);
            }
        }
    }
}

fn raster_blit_image(
    backend: &mut BitmapRenderer,
    rect: &Rect,
    bmp: &Bitmap,
    repeat: bool,
    offset: (i32, i32),
) {
    if rect.width == 0 || rect.height == 0 || backend.width == 0 {
        return;
    }
    let x0 = min(rect.x.max(0) as usize, backend.width);
    let y0 = min(rect.y.max(0) as usize, backend.height);
    let x1 = min(x0 + rect.width as usize, backend.width);
    let y1 = min(y0 + rect.height as usize, backend.height);
    for yy in y0..y1 {
        let row = yy * backend.width;
        for xx in x0..x1 {
            let sample_x = (xx as i32 - rect.x + offset.0) as usize;
            let sample_y = (yy as i32 - rect.y + offset.1) as usize;
            let color = if repeat {
                bmp.sample(sample_x, sample_y)
            } else if let Some(c) = bmp.pixel(sample_x, sample_y) {
                c
            } else {
                continue;
            };
            backend.storage[row + xx] = color;
        }
    }
}

fn raster_draw_text(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    text: &str,
    color: Rgba,
    max_width: Option<u32>,
) {
    if backend.width == 0 {
        return;
    }
    let mut cursor_x = origin.0.max(0) as usize;
    let mut cursor_y = origin.1.max(0) as usize;
    let limit_x = max_width.map(|w| cursor_x + w as usize);
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = origin.0.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width();
        if let Some(limit) = limit_x {
            if cursor_x + gw > limit {
                break;
            }
        }
        raster_draw_glyph(backend, cursor_x, cursor_y, glyph, color.to_u32());
        cursor_x += gw;
    }
}

fn raster_draw_text_block(backend: &mut BitmapRenderer, rect: &Rect, text: &str, color: Rgba) {
    if backend.width == 0 {
        return;
    }
    let mut cursor_x = rect.x.max(0) as usize;
    let mut cursor_y = rect.y.max(0) as usize;
    let max_x = rect.x.max(0) as usize + rect.width as usize;
    let max_y = rect.y.max(0) as usize + rect.height as usize;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = rect.x.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            if cursor_y + FONT_HEIGHT >= max_y {
                break;
            }
            continue;
        }
        let Some(glyph) = get_glyph(ch) else { continue };
        let gw = glyph.get_width();
        if cursor_x + gw >= max_x {
            cursor_x = rect.x.max(0) as usize;
            cursor_y = cursor_y.saturating_add(FONT_HEIGHT);
            if cursor_y + FONT_HEIGHT >= max_y {
                break;
            }
        }
        raster_draw_glyph(backend, cursor_x, cursor_y, glyph, color.to_u32());
        cursor_x += gw;
    }
}

fn raster_draw_glyph(
    backend: &mut BitmapRenderer,
    x: usize,
    y: usize,
    glyph: &unifont::Glyph,
    color: u32,
) {
    if x >= backend.width || y >= backend.height || backend.width == 0 {
        return;
    }

    let width = glyph.get_width();
    for row in 0..FONT_HEIGHT {
        let dst_y = y + row;
        if dst_y >= backend.height {
            break;
        }
        for col in 0..width {
            let dst_x = x + col;
            if dst_x >= backend.width {
                break;
            }
            if glyph.get_pixel(col, row) {
                let idx = dst_y * backend.width + dst_x;
                backend.storage[idx] = color;
            }
        }
    }
}

fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

fn dist_sq_segment(px: i32, py: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> u64 {
    let px = px as i64;
    let py = py as i64;
    let x1 = x1 as i64;
    let y1 = y1 as i64;
    let x2 = x2 as i64;
    let y2 = y2 as i64;

    let dx = x2 - x1;
    let dy = y2 - y1;

    if dx == 0 && dy == 0 {
        return ((px - x1).pow(2) + (py - y1).pow(2)) as u64;
    }

    let num = (px - x1) * dx + (py - y1) * dy;
    let den = dx * dx + dy * dy;

    if num <= 0 {
        return ((px - x1).pow(2) + (py - y1).pow(2)) as u64;
    }
    if num >= den {
        return ((px - x2).pow(2) + (py - y2).pow(2)) as u64;
    }

    let r_x = (px - x1) * den - num * dx;
    let r_y = (py - y1) * den - num * dy;

    let dist_sq_scaled = r_x * r_x + r_y * r_y;
    (dist_sq_scaled / (den * den)) as u64
}

fn get_distance_and_sign(px: i32, py: i32, poly: &[(i32, i32)]) -> (u32, bool) {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    let mut min_dist_sq = u64::MAX;

    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];

        let num = (py - yi) * (xj - xi);
        let den = yj - yi;

        if (yi > py) != (yj > py) {
            let lhs = (px - xi) as i64 * den as i64;
            let rhs = num as i64;
            if den > 0 {
                if lhs < rhs {
                    inside = !inside;
                }
            } else {
                if lhs > rhs {
                    inside = !inside;
                }
            }
        }

        let d2 = dist_sq_segment(px, py, xi, yi, xj, yj);
        if d2 < min_dist_sq {
            min_dist_sq = d2;
        }

        j = i;
    }

    (isqrt(min_dist_sq) as u32, inside)
}

fn clamp_unit(val: i32, scale: i32) -> u32 {
    if val <= 0 {
        0
    } else if val >= scale {
        255
    } else {
        (val as u32 * 255) / scale as u32
    }
}

fn raster_draw_cursor(
    backend: &mut BitmapRenderer,
    origin: (i32, i32),
    primary: Rgba,
    shadow: Rgba,
    pressed: bool,
) {
    if backend.width == 0 {
        return;
    }

    // Largish vector cursor
    let scale = 16;
    let poly = [
        (0 * scale, 0 * scale),
        (0 * scale, 30 * scale),
        (8 * scale, 22 * scale),
        (14 * scale, 34 * scale),
        (18 * scale, 32 * scale),
        (12 * scale, 20 * scale),
        (22 * scale, 20 * scale),
    ];

    let mut max_x = 0;
    let mut max_y = 0;
    for &(px, py) in &poly {
        if px > max_x {
            max_x = px;
        }
        if py > max_y {
            max_y = py;
        }
    }
    // Convert back to pixels for bounds
    max_x /= scale;
    max_y /= scale;

    let shadow_offset_x = 2;
    let shadow_offset_y = 2;
    let draw_shadow = !pressed;
    let outline_thickness = 1;

    let min_rel_x = -outline_thickness - 1; // Extra padding for AA
    let min_rel_y = -outline_thickness - 1;
    let max_rel_x = max_x
        + if draw_shadow {
            shadow_offset_x
        } else {
            outline_thickness
        }
        + 2;
    let max_rel_y = max_y
        + if draw_shadow {
            shadow_offset_y
        } else {
            outline_thickness
        }
        + 2;

    let base_x = origin.0;
    let base_y = origin.1;

    let start_x = clamp_i32(base_x + min_rel_x, 0, backend.width as i32);
    let start_y = clamp_i32(base_y + min_rel_y, 0, backend.height as i32);
    let end_x = clamp_i32(base_x + max_rel_x, 0, backend.width as i32);
    let end_y = clamp_i32(base_y + max_rel_y, 0, backend.height as i32);

    // Inverse color for outline
    let outline_color = Rgba::new(0xff, 255 - primary.r, 255 - primary.g, 255 - primary.b);

    for y in start_y..end_y {
        for x in start_x..end_x {
            let local_x = x - base_x;
            let local_y = y - base_y;

            let px_center_x = local_x * scale + scale / 2;
            let px_center_y = local_y * scale + scale / 2;

            let (dist, inside) = get_distance_and_sign(px_center_x, px_center_y, &poly);
            let signed_dist = if inside { dist as i32 } else { -(dist as i32) };

            // Primary coverage: ramp from -0.5 to 0.5 pixels (-8 to 8 units)
            let alpha_primary = clamp_unit(signed_dist + scale / 2, scale);

            // Outline coverage: ramp from -1.5 to -0.5 pixels relative to edge?
            // Outline is from -16 to 0.
            // We want coverage of shape expanded by 16 units.
            // Expanded shape boundary is at -16.
            // Ramp centered at -16.
            // alpha_total = clamp(signed_dist + 16 + 8, 16)
            let alpha_total = clamp_unit(signed_dist + scale + scale / 2, scale);

            let alpha_outline = alpha_total.saturating_sub(alpha_primary);

            // Shadow coverage
            let mut alpha_shadow = 0;
            if draw_shadow {
                let shadow_dx = shadow_offset_x * scale;
                let shadow_dy = shadow_offset_y * scale;
                let (s_dist, s_inside) =
                    get_distance_and_sign(px_center_x - shadow_dx, px_center_y - shadow_dy, &poly);
                let s_signed_dist = if s_inside {
                    s_dist as i32
                } else {
                    -(s_dist as i32)
                };
                alpha_shadow = clamp_unit(s_signed_dist + scale / 2, scale);
            }

            let idx = (y as usize) * backend.width + (x as usize);
            let mut color = backend.storage[idx];

            // Composite: Background -> Shadow -> Outline -> Primary

            if alpha_shadow > 0 {
                let sa = (shadow.a as u32 * alpha_shadow) / 255;
                if sa > 0 {
                    let shadow_rgb = shadow.to_u32() & 0x00FFFFFF;
                    let s_color = (sa << 24) | shadow_rgb;
                    color = userland::graphics::blend(s_color, color);
                }
            }

            if alpha_outline > 0 {
                let oa = (outline_color.a as u32 * alpha_outline) / 255;
                if oa > 0 {
                    let o_color = (oa << 24) | (outline_color.to_u32() & 0x00FFFFFF);
                    color = userland::graphics::blend(o_color, color);
                }
            }

            if alpha_primary > 0 {
                let pa = (primary.a as u32 * alpha_primary) / 255;
                if pa > 0 {
                    let p_color = (pa << 24) | (primary.to_u32() & 0x00FFFFFF);
                    color = userland::graphics::blend(p_color, color);
                }
            }

            backend.storage[idx] = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{boxed::Box, vec};

    #[test]
    fn bitmap_pixel_respects_bounds() {
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        assert_eq!(bmp.pixel(0, 0), Some(1));
        assert_eq!(bmp.pixel(1, 1), Some(4));
        assert_eq!(bmp.pixel(2, 0), None);
        assert_eq!(bmp.pixel(0, 2), None);
    }

    #[test]
    fn raster_blit_image_skips_out_of_bounds_when_not_repeating() {
        let mut backend = BitmapRenderer::new(3, 3);
        let bmp = Bitmap::new(2, 2, vec![1, 2, 3, 4]);

        raster_blit_image(&mut backend, &Rect::new(0, 0, 3, 3), &bmp, false);

        assert_eq!(backend.storage, &[1, 2, 0, 3, 4, 0, 0, 0, 0]);
    }
}
