use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use userland::println;

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Arc<[u32]>,
}

impl Bitmap {
    pub fn new(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        if pixels.len() != width * height {
            panic!(
                "Bitmap::new: pixels length {} does not match width {} * height {}",
                pixels.len(),
                width,
                height
            );
        }
        Self {
            width,
            height,
            pixels: pixels.into(),
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(self.pixels[y * self.width + x])
        }
    }

    pub fn sample(&self, x: usize, y: usize) -> u32 {
        if self.width == 0 || self.height == 0 {
            return 0;
        }
        let sx = x % self.width;
        let sy = y % self.height;
        self.pixels[sy * self.width + sx]
    }
}

pub fn load_background() -> Arc<Bitmap> {
    let data = include_bytes!("../../clouds.bmp");
    Arc::new(decode_bmp(data).unwrap_or_else(|| fallback_background()))
}

pub fn fallback_background() -> Bitmap {
    let width = 64;
    let height = 64;
    let mut pixels = Vec::with_capacity(width * height);
    for _y in 0..height {
        for _x in 0..width {
            let color = 0xFF000000; // Black
            pixels.push(color);
        }
    }
    Bitmap::new(width, height, pixels)
}

pub fn decode_bmp(data: &[u8]) -> Option<Bitmap> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return None;
    }
    let data_offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(data[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(data[22..26].try_into().ok()?);
    let planes = u16::from_le_bytes(data[26..28].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[28..30].try_into().ok()?);
    let compression = u32::from_le_bytes(data[30..34].try_into().ok()?);

    if planes != 1 || (bpp != 24 && bpp != 32) || compression != 0 {
        return None;
    }

    let width_u = width.unsigned_abs() as usize;
    let height_u = height.unsigned_abs() as usize;
    let stride = if bpp == 24 {
        ((width_u * 3 + 3) / 4) * 4
    } else {
        width_u * 4
    };

    if data_offset + stride.saturating_mul(height_u) > data.len() {
        return None;
    }

    let mut pixels = vec![0u32; width_u * height_u];
    println!(
        "Allocated pixels at {:p} size {}x{}",
        pixels.as_ptr(),
        width_u,
        height_u
    );
    for row in 0..height_u {
        let src_row = if height > 0 { height_u - 1 - row } else { row };
        let src_start = data_offset + src_row * stride;
        for col in 0..width_u {
            if bpp == 24 {
                let idx = src_start + col * 3;
                if idx + 3 > data.len() {
                    break;
                }
                let b = data[idx] as u32;
                let g = data[idx + 1] as u32;
                let r = data[idx + 2] as u32;
                pixels[row * width_u + col] = 0xFF000000 | (r << 16) | (g << 8) | b;
            } else {
                let idx = src_start + col * 4;
                if idx + 4 > data.len() {
                    break;
                }
                let b = data[idx] as u32;
                let g = data[idx + 1] as u32;
                let r = data[idx + 2] as u32;
                let a = data[idx + 3] as u32;
                pixels[row * width_u + col] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
    }

    Some(Bitmap::new(width_u, height_u, pixels))
}
