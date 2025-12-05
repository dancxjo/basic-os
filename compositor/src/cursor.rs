use crate::bitmap::Bitmap;
use crate::types::{
    clamp_i32, Rgba, COLOR_CURSOR_PRIMARY, COLOR_CURSOR_SHADOW, CURSOR_SIZE, THEME,
};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Arrow,
    Move,
    ResizeN,
    ResizeS,
    ResizeE,
    ResizeW,
    ResizeNE,
    ResizeNW,
    ResizeSE,
    ResizeSW,
}

#[derive(Clone)]
pub struct CursorIcon {
    pub bitmap: Arc<Bitmap>,
    pub hotspot: (i32, i32),
}

pub struct CursorSprites {
    pub arrow: CursorIcon,
    pub move_icon: CursorIcon,
    pub resize_ns: CursorIcon,
    pub resize_ew: CursorIcon,
    pub resize_ne_sw: CursorIcon,
    pub resize_nw_se: CursorIcon,
}

impl CursorSprites {
    pub fn for_kind(&self, kind: CursorKind) -> &CursorIcon {
        match kind {
            CursorKind::Arrow => &self.arrow,
            CursorKind::Move => &self.move_icon,
            CursorKind::ResizeN | CursorKind::ResizeS => &self.resize_ns,
            CursorKind::ResizeE | CursorKind::ResizeW => &self.resize_ew,
            CursorKind::ResizeNE | CursorKind::ResizeSW => &self.resize_ne_sw,
            CursorKind::ResizeNW | CursorKind::ResizeSE => &self.resize_nw_se,
        }
    }
}

pub struct CursorState {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub visible: bool,
    pub kind: CursorKind,
}

impl CursorState {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            x: (width / 2) as i32,
            y: (height / 2) as i32,
            buttons: 0,
            visible: true,
            kind: CursorKind::Arrow,
        }
    }

    pub fn update(&mut self, dx: i64, dy: i64, buttons: u8, width: usize, height: usize) {
        self.x = clamp_i32(self.x + dx as i32, 0, width.saturating_sub(1) as i32);
        self.y = clamp_i32(self.y + dy as i32, 0, height.saturating_sub(1) as i32);
        self.buttons = buttons;
    }

    pub fn set_from_graph(
        &mut self,
        x: Option<i64>,
        y: Option<i64>,
        visible: Option<bool>,
        width: usize,
        height: usize,
    ) {
        if let Some(nx) = x {
            self.x = clamp_i32(nx as i32, 0, width.saturating_sub(1) as i32);
        }
        if let Some(ny) = y {
            self.y = clamp_i32(ny as i32, 0, height.saturating_sub(1) as i32);
        }
        if let Some(vis) = visible {
            self.visible = vis;
        }
    }

    pub fn set_kind(&mut self, kind: CursorKind) {
        self.kind = kind;
    }
}

struct CursorMask {
    width: usize,
    height: usize,
    hotspot: (i32, i32),
    data: Vec<bool>,
}

impl CursorMask {
    fn new(width: usize, height: usize, hotspot: (i32, i32)) -> Self {
        Self {
            width,
            height,
            hotspot,
            data: vec![false; width * height],
        }
    }

    fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux < self.width && uy < self.height {
            self.data[uy * self.width + ux] = true;
        }
    }

    fn filled(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux >= self.width || uy >= self.height {
            return false;
        }
        self.data[uy * self.width + ux]
    }
}

fn fill_triangle(mask: &mut CursorMask, p1: (i32, i32), p2: (i32, i32), p3: (i32, i32)) {
    let min_x = core::cmp::min(p1.0, core::cmp::min(p2.0, p3.0));
    let max_x = core::cmp::max(p1.0, core::cmp::max(p2.0, p3.0));
    let min_y = core::cmp::min(p1.1, core::cmp::min(p2.1, p3.1));
    let max_y = core::cmp::max(p1.1, core::cmp::max(p2.1, p3.1));

    let area_sign = |a: (i32, i32), b: (i32, i32), c: (i32, i32)| -> i32 {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x, y);
            let d1 = area_sign(p, p2, p3);
            let d2 = area_sign(p, p3, p1);
            let d3 = area_sign(p, p1, p2);

            let has_neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
            let has_pos = (d1 > 0) || (d2 > 0) || (d3 > 0);

            if !(has_neg && has_pos) {
                mask.set(x, y);
            }
        }
    }
}

fn cursor_icon_from_mask(mask: CursorMask, fill: Rgba, outline: Rgba, _shadow: Rgba) -> CursorIcon {
    let w = mask.width;
    let h = mask.height;
    let mut pixels = vec![0u32; w * h];

    // 1. Generate shadow map (alpha values)
    let mut shadow_alpha = vec![0u8; w * h];
    let offset_x = 4;
    let offset_y = 4;

    for y in 0..h {
        for x in 0..w {
            if mask.data[y * w + x] {
                let sx = x + offset_x;
                let sy = y + offset_y;
                if sx < w && sy < h {
                    shadow_alpha[sy * w + sx] = 0xA0; // Initial intensity
                }
            }
        }
    }

    // 2. Blur the shadow map (simple box blur, 3 passes)
    for _ in 0..3 {
        let src = shadow_alpha.clone();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let mut sum: u32 = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        sum +=
                            src[(y as isize + dy) as usize * w + (x as isize + dx) as usize] as u32;
                    }
                }
                shadow_alpha[y * w + x] = (sum / 9) as u8;
            }
        }
    }

    // 3. Composite shadow
    for i in 0..pixels.len() {
        let a = shadow_alpha[i];
        if a > 0 {
            // Black shadow
            pixels[i] = (a as u32) << 24;
        }
    }

    // 4. Composite cursor shape
    for y in 0..h {
        for x in 0..w {
            if !mask.data[y * w + x] {
                continue;
            }
            let neighbors = [
                (x as i32 - 1, y as i32),
                (x as i32 + 1, y as i32),
                (x as i32, y as i32 - 1),
                (x as i32, y as i32 + 1),
            ];
            let is_edge = neighbors.iter().any(|(nx, ny)| !mask.filled(*nx, *ny));
            let color = if is_edge { outline } else { fill };

            // Premultiply alpha
            let a = color.a as u32;
            let r = (color.r as u32 * a) / 255;
            let g = (color.g as u32 * a) / 255;
            let b = (color.b as u32 * a) / 255;
            pixels[y * w + x] = (a << 24) | (r << 16) | (g << 8) | b;
        }
    }

    CursorIcon {
        bitmap: Arc::new(Bitmap::new(mask.width, mask.height, pixels)),
        hotspot: mask.hotspot,
    }
}

fn make_arrow_mask() -> CursorMask {
    let _size = CURSOR_SIZE as i32;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (0, 0));

    // Standard arrow shape scaled 3x
    // Main triangle
    fill_triangle(&mut mask, (0, 0), (0, 45), (33, 33));

    // Stem
    // A quad from (12, 36), (21, 54), (27, 51), (18, 33)
    fill_triangle(&mut mask, (12, 36), (21, 54), (27, 51));
    fill_triangle(&mut mask, (12, 36), (27, 51), (18, 33));

    mask
}

fn make_move_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    // Thicker cross
    for y in (c - 14)..=(c + 14) {
        for x in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for x in (c - 14)..=(c + 14) {
        for y in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    // Arrow heads
    for i in 0..14 {
        for dx in -i..=i {
            mask.set(c + dx, c - 15 - i);
            mask.set(c + dx, c + 15 + i);
            mask.set(c - 15 - i, c + dx);
            mask.set(c + 15 + i, c + dx);
        }
    }
    mask
}

fn make_resize_ns_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for y in (c - 14)..=(c + 14) {
        for x in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for i in 0..14 {
        for dx in -i..=i {
            mask.set(c + dx, c - 15 - i);
            mask.set(c + dx, c + 15 + i);
        }
    }
    mask
}

fn make_resize_ew_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for x in (c - 14)..=(c + 14) {
        for y in (c - 5)..=(c + 5) {
            mask.set(x, y);
        }
    }
    for i in 0..14 {
        for dy in -i..=i {
            mask.set(c - 15 - i, c + dy);
            mask.set(c + 15 + i, c + dy);
        }
    }
    mask
}

fn make_resize_nw_se_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for offset in -18..=18 {
        let x = c + offset;
        let y = c + offset;
        for t in -4..=4 {
            mask.set(x + t, y);
        }
    }
    for i in 0..14 {
        for dx in 0..=i {
            mask.set(c - 19 - i, c - 4 + dx);
            mask.set(c - 4 + dx, c - 19 - i);
            mask.set(c + 19 + i, c + 4 - dx);
            mask.set(c + 4 - dx, c + 19 + i);
        }
    }
    mask
}

fn make_resize_ne_sw_mask() -> CursorMask {
    let size = CURSOR_SIZE as i32;
    let c = size / 2;
    let mut mask = CursorMask::new(CURSOR_SIZE, CURSOR_SIZE, (c, c));

    for offset in -18..=18 {
        let x = c + offset;
        let y = c - offset;
        for t in -4..=4 {
            mask.set(x + t, y);
        }
    }
    for i in 0..14 {
        for dx in 0..=i {
            mask.set(c + 19 + i, c - 4 - dx);
            mask.set(c + 4 + dx, c - 19 - i);
            mask.set(c - 19 - i, c + 4 + dx);
            mask.set(c - 4 - dx, c + 19 + i);
        }
    }
    mask
}

pub fn build_cursor_sprites() -> CursorSprites {
    let outline = THEME.frame_outer;
    let fill = COLOR_CURSOR_PRIMARY;
    let shadow = COLOR_CURSOR_SHADOW;

    let arrow = cursor_icon_from_mask(make_arrow_mask(), fill, outline, shadow);
    let move_icon = cursor_icon_from_mask(make_move_mask(), fill, outline, shadow);
    let resize_ns = cursor_icon_from_mask(make_resize_ns_mask(), fill, outline, shadow);
    let resize_ew = cursor_icon_from_mask(make_resize_ew_mask(), fill, outline, shadow);
    let resize_nw_se = cursor_icon_from_mask(make_resize_nw_se_mask(), fill, outline, shadow);
    let resize_ne_sw = cursor_icon_from_mask(make_resize_ne_sw_mask(), fill, outline, shadow);

    CursorSprites {
        arrow,
        move_icon,
        resize_ns,
        resize_ew,
        resize_ne_sw,
        resize_nw_se,
    }
}
