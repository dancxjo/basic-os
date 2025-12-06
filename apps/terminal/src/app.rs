use alloc::string::{String, ToString};
use alloc::vec::Vec;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter, Value};
use userland::uuid::Uuid;

const MAX_LINES: usize = 200;

pub struct Terminal {
    window: WindowHandle,
    lines: Vec<String>,
    #[allow(dead_code)]
    watch_id: WatchId,
    redraw_needed: bool,
}

impl App for Terminal {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        // Create a unique place_id for terminal mode
        let place_id = userland::simple_uuid(b"terminal_place");
        
        // Create window as root for mode 11 (F12)
        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Terminal".to_string(),
            x: 0,
            y: 0,
            width: 1024,
            height: 768,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            is_place_root: true,
            place_id: Some(place_id),
            mode_index: Some(11), // F12 = mode 11 (0-indexed: F1=0...F12=11)
            window_rect: None,
            gap: None,
            flex_direction: None,
            justify_content: None,
            align_items: None,
            tile_mode: None,
        };
        
        let window = ctx.create_window_with(window_fields);

        // Set styling
        ctx.set_font_mono(&window, true);
        ctx.set_bg_color(&window, 0xFF202020);
        ctx.set_text_color(&window, 0xFF4DB8FF);

        // Subscribe to DEBUG_LOG things
        let watch_id = ctx.watch_graph(ThingFilter {
            kind: Some(canon::DEBUG_LOG),
            id: None,
        });

        let mut terminal = Terminal {
            window,
            lines: Vec::new(),
            watch_id,
            redraw_needed: true,
        };

        terminal.lines.push(String::from("═══════════════════════════════════════════"));
        terminal.lines.push(String::from("  ThingOS Terminal - Debug Output Console"));
        terminal.lines.push(String::from("═══════════════════════════════════════════"));
        terminal.lines.push(String::from(""));
        terminal
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::DEBUG_LOG {
                // Extract message from the debug log thing
                if let Some(Value::Text(msg)) = thing.fields.get(&canon::TEXT) {
                    self.append_line(msg);
                }
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        if self.redraw_needed {
            ctx.clear_window(&self.window);
            
            for line in &self.lines {
                ctx.draw_text(&self.window, format_args!("{}\n", line));
            }
            
            self.redraw_needed = false;
        }
    }
}

impl Terminal {
    fn append_line(&mut self, msg: &str) {
        self.lines.push(String::from(msg));
        
        // Keep only the last MAX_LINES
        while self.lines.len() > MAX_LINES {
            self.lines.remove(0);
        }
        
        self.redraw_needed = true;
    }
}
