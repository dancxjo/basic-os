#![no_std]

extern crate alloc;

use userland::prelude::*;

pub struct HelloApp {
    window: WindowHandle,
}

impl App for HelloApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Hello");
        HelloApp { window }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        if tick % 8 != 0 {
            return;
        }
        ctx.clear_window(&self.window);
        ctx.draw_text(
            &self.window,
            format_args!(
                "Hello from app {}\nrev {}\nEnjoy the clouds.",
                "hello", tick
            ),
        );
    }
}

app_main!(HelloApp);
