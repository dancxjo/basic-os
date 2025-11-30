use userland::prelude::*;
use userland::{canon, AppEvent, ThingFilter};

pub struct CloudsApp {
    window: WindowHandle,
    bmp_data: &'static [u8],
    key_count: usize,
    sent_bitmap: bool,
}

impl App for CloudsApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Clouds");

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_PRESSED),
            id: None,
        });

        let bmp_data = include_bytes!("../../../clouds.bmp");

        CloudsApp {
            window,
            bmp_data,
            key_count: 0,
            sent_bitmap: false,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::KEY_PRESSED {
                self.key_count += 1;
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        ctx.clear_window(&self.window);

        if !self.sent_bitmap {
            ctx.draw_bitmap(&self.window, self.bmp_data);
            self.sent_bitmap = true;
        }

        ctx.draw_text(
            &self.window,
            format_args!("Keys pressed: {}", self.key_count),
        );
    }
}
