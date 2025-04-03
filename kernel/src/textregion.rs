use crate::{
    framebuffer::Framebuffer,
    input::Modifiers,
    keymaps::{KeyMapEntry, US_ALTGR_INTL},
    textbox::TextBox,
};

pub struct TextRegion {
    pub window: crate::ui::Window,
    pub textbox: TextBox,
}

impl TextRegion {
    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        let window = crate::ui::Window::new(x, y, width, height);
        let textbox = TextBox::new(x + 4, y + 4, width - 8, height - 8);
        Self { window, textbox }
    }

    pub fn draw(&self, fb: &mut Framebuffer) {
        self.window.draw(fb);
        self.textbox.draw(fb);
    }

    pub fn insert_key(&mut self, keycode: u8, modifiers: &Modifiers) -> Option<char> {
        if keycode == 28 {
            self.textbox.insert_char('\n'); // Handle Enter key
            return Some('\n');
        }

        if keycode as usize >= US_ALTGR_INTL.len() {
            return None;
        }

        let entry: KeyMapEntry = US_ALTGR_INTL[keycode as usize];
        let ch = match (modifiers.shift, modifiers.altgr) {
            (true, true) => entry.shift_altgr,
            (false, true) => entry.altgr,
            (true, false) => entry.shifted,
            (false, false) => entry.normal,
        };

        if let Some(c) = ch {
            self.textbox.insert_char(c);
        }
        ch
    }

    pub fn scroll_up(&mut self) {
        self.textbox.scroll_up();
    }

    pub fn scroll_down(&mut self) {
        self.textbox.scroll_down();
    }
}
