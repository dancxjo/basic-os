
fn draw_pixel(fb: &mut [u8], rect: Rect, x: i32, y: i32, color: u32) {
    let dst_x = rect.x + x;
    let dst_y = rect.y + y;
    if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 || dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
        return;
    }
    let offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
    if offset + 4 <= fb.len() {
        fb[offset] = (color >> 16) as u8;
        fb[offset + 1] = (color >> 8) as u8;
        fb[offset + 2] = color as u8;
        fb[offset + 3] = (color >> 24) as u8;
    }
}

fn draw_rect_outline(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: i32, h: i32, color: u32) {
    draw_line_h(fb, rect, x, y, w, color);
    draw_line_h(fb, rect, x, y + h - 1, w, color);
    draw_line_v(fb, rect, x, y, h, color);
    draw_line_v(fb, rect, x + w - 1, y, h, color);
}

fn draw_line_h(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: i32, color: u32) {
    for i in 0..w {
        let dst_x = rect.x + x + i;
        let dst_y = rect.y + y;
        if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 || dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
            continue;
        }
        let offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
        if offset + 4 <= fb.len() {
            fb[offset] = (color >> 16) as u8;
            fb[offset + 1] = (color >> 8) as u8;
            fb[offset + 2] = color as u8;
            fb[offset + 3] = (color >> 24) as u8;
        }
    }
}

fn draw_line_v(fb: &mut [u8], rect: Rect, x: i32, y: i32, h: i32, color: u32) {
    for i in 0..h {
        let dst_x = rect.x + x;
        let dst_y = rect.y + y + i;
        if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 || dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
            continue;
        }
        let offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
        if offset + 4 <= fb.len() {
            fb[offset] = (color >> 16) as u8;
            fb[offset + 1] = (color >> 8) as u8;
            fb[offset + 2] = color as u8;
            fb[offset + 3] = (color >> 24) as u8;
        }
    }
}
