use alloc::string::ToString;
use userland::prelude::*;
use userland::uuid::Uuid;
use userland::{canon, graph, AppEvent};

pub struct Terminal {
    window: WindowHandle,
}

impl App for Terminal {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let mut window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Terminal".to_string(),
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            mode_index: Some(11), // F12
            window_rect: None,
        };
        let window = ctx.create_window_with(window_fields);

        // Draw some background
        ctx.clear_window(&window);
        ctx.draw_text(&window, format_args!("Terminal (F12 Root)\n"));

        Terminal { window }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        if tick % 60 == 0 {
            ctx.draw_text(&self.window, format_args!("Tick: {}\n", tick));
        }
    }
}
