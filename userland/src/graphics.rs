pub fn blend(fg: u32, bg: u32) -> u32 {
    let alpha = (fg >> 24) & 0xFF;
    if alpha == 0 {
        return bg;
    }
    if alpha == 255 {
        return fg;
    }

    let inv_alpha = 255 - alpha;

    let r = (((fg >> 16) & 0xFF) * alpha + ((bg >> 16) & 0xFF) * inv_alpha) / 255;
    let g = (((fg >> 8) & 0xFF) * alpha + ((bg >> 8) & 0xFF) * inv_alpha) / 255;
    let b = ((fg & 0xFF) * alpha + (bg & 0xFF) * inv_alpha) / 255;

    0xFF000000 | (r << 16) | (g << 8) | b
}
