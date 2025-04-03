use crate::{framebuffer::Framebuffer, keymaps::US_ALTGR_INTL};

pub struct Window {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Window {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        fb.draw_rect(self.x, self.y, self.width, self.height, 0x222244);
    }
}

pub struct GlyphTrail {
    pub x: usize,
    pub y: usize,
    pub cursor: usize,
    pub active_dead_key: Option<char>,
    pub compose_table: crate::keymaps::ComposeTable,
}

impl GlyphTrail {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            x,
            y,
            cursor: 0,
            active_dead_key: None,
            compose_table: crate::keymaps::ComposeTable::new(),
        }
    }

    pub fn advance(&mut self) {
        self.cursor += 1;
    }

    pub fn draw_block(&mut self, fb: &mut Framebuffer, key: u8) {
        let block_size = 8;
        let cx = self.x + self.cursor * (block_size + 1);
        if cx > self.x + fb.width() {
            self.x = 0;
            self.y += block_size + 1;
        }
        let cy = self.y;
        let key = US_ALTGR_INTL[key as usize];

        if let Some(c) = key.normal {
            if let Some(dead) = self.active_dead_key {
                if let Some(composed) = self.compose_table.try_compose(dead, c) {
                    fb.draw_char(cx, cy, composed, 0x00FF00);
                    self.active_dead_key = None;
                } else {
                    fb.draw_char(cx, cy, dead, 0x00FF00);
                    self.advance();
                    fb.draw_char(cx + 9, cy, c, 0x00FF00);
                    self.active_dead_key = None;
                }
            } else if Self::is_dead_key(c) {
                self.active_dead_key = Some(c);
            } else {
                fb.draw_char(cx, cy, c, 0x00FF00);
            }
        }
    }

    fn is_dead_key(c: char) -> bool {
        matches!(c, '´' | '`' | '^' | '~' | '"' | ',')
    }
}
