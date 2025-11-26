#![no_std]

extern crate alloc;

use userland::prelude::*;

pub struct ClockApp {
    window: WindowHandle,
}

impl App for ClockApp {
    fn init(ctx: &mut AppContext) -> Self {
        let window = ctx.create_window("Clock");
        ClockApp { window }
    }

    fn tick(&mut self, ctx: &mut AppContext, tick: u64) {
        if tick % 4 != 0 {
            return;
        }
        let seconds = tick / 4;
        let minutes = seconds / 60;
        let hours = minutes / 60;

        ctx.clear_window(&self.window);
        ctx.draw_text(
            &self.window,
            format_args!(
                "Clock\n{:02}:{:02}:{:02}\nframe {}",
                hours % 24,
                minutes % 60,
                seconds % 60,
                tick
            ),
        );
    }
}

app_main!(ClockApp);
