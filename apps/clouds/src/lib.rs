#![no_std]

extern crate alloc;
use userland::prelude::*;

pub struct CloudsApp {
    window: WindowHandle,
}

impl App for CloudsApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Clouds");
        CloudsApp { window }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        static CLOUD_FRAMES: [&str; 2] = [
            "~~  ~ ~~~     ~~~\n ~~~   ~~  ~~ ~~  \n   ~~~ ~   ~~   ~~",
            " ~~~ ~   ~~   ~~ \n~~  ~~~  ~~~   ~~ \n   ~~~ ~~~ ~   ~~ ",
        ];
        if tick % 16 != 0 {
            return;
        }
        let frame = CLOUD_FRAMES[(tick as usize / 16) % CLOUD_FRAMES.len()];
        ctx.clear_window(&self.window);
        ctx.draw_text(&self.window, format_args!("{}", frame));
    }
}

app_main!(CloudsApp);
