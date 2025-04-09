use crate::framebuffer::Framebuffer;
use crate::keymaps::ComposeTable;
use alloc::vec::Vec;

/// A text box that flows like a Publisher-style region.
pub struct TextBox {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub line_height: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub content: Vec<char>,
    pub compose: Option<char>,
    pub table: ComposeTable,
    pub scroll_offset: usize,
    pub cursor_visible: bool,
    pub last_toggle: u64, // e.g., timestamp in milliseconds
}

impl TextBox {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
            line_height: 18,
            cursor_col: 0,
            cursor_row: 0,
            content: Vec::new(),
            compose: None,
            table: ComposeTable::new(),
            scroll_offset: 0,
            cursor_visible: true,
            last_toggle: 0,
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if let Some(dead) = self.compose {
            if let Some(composed) = self.table.try_compose(dead, ch) {
                self.content.push(composed);
            } else {
                self.content.push(dead);
                self.content.push(ch);
            }
            self.compose = None;
        } else if Self::is_dead_key(ch) {
            self.compose = Some(ch);
        } else {
            self.content.push(ch);
            if ch == '\n' {
                self.cursor_col = 0;
                self.cursor_row += 1;
            } else {
                self.cursor_col += 1;
                if self.cursor_col * 9 > self.width {
                    self.cursor_col = 0;
                    self.cursor_row += 1;
                }
            }
        }
    }

    pub fn insert_string(&mut self, s: &str) {
        for ch in s.chars() {
            self.insert_char(ch);
        }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        let x = self.x;
        let y = self.y;
        let line_height = self.line_height;

        let chars_per_line = self.width / 9;
        let max_lines = self.height / line_height;
        let approx_chars_visible = chars_per_line * max_lines;

        let start_index = self.content.len().saturating_sub(approx_chars_visible);

        let mut col = 0;
        let mut row = 0;

        for &ch in self.content[start_index..].iter() {
            if ch == '\n' {
                col = 0;
                row += 1;
                continue;
            }

            if row < max_lines {
                fb.draw_char(x + col * 9, y + row * line_height, ch, 0x00FF00);
            }

            col += 1;
            if col * 9 > self.width {
                col = 0;
                row += 1;
            }

            if row >= max_lines {
                break;
            }
        }

        // Draw cursor if visible and fits on screen
        if self.cursor_visible && self.cursor_row >= self.cursor_row.saturating_sub(max_lines) {
            fb.draw_char(
                x + self.cursor_col * 9,
                y + (row.min(max_lines - 1)) * line_height,
                '_',
                0xFFFFFF,
            );
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    fn is_dead_key(c: char) -> bool {
        matches!(c, '´' | '`' | '^' | '~' | '"' | ',')
    }
}
