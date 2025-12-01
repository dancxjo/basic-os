use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, Symbol, ThingFilter};
use uuid::Uuid;

const SMOKE_KIND: Symbol = canon::canon(b'G', b'S', b'M');
const NODE_NAME: &str = "graph-smoke-node";
const MAX_KEY_LOG: usize = 24;

pub struct DemoApp {
    window: WindowHandle,
    key_count: usize,
    key_log: VecDeque<String>,
    // Smoke state
    smoke_node: Uuid,
    smoke_watch: WatchId,
    smoke_seen_events: bool,
}

impl App for DemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Demo Application");

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_PRESSED),
            id: None,
        });
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        let bmp_data = create_demo_image_data();

        // Create Image Widget
        let image_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"demo_image");
        let mut fields = graph::map();
        fields.insert(canon::cc('W', 'K'), Value::Text(String::from("image")));
        fields.insert(canon::WIDTH, Value::U64(16));
        fields.insert(canon::HEIGHT, Value::U64(16));
        fields.insert(canon::cc('I', 'D'), Value::Bytes(bmp_data));
        fields.insert(canon::PARENT, Value::Uuid(window.window_id()));
        
        graph::fiat(Some(image_id), canon::WIDGET, fields);
        
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        graph::grant_capability(widget_host_bundle, image_id, "CAN_READ");
        
        graph::that(window.window_id(), "contains", image_id, 0);

        // --- Graph Client Logic ---
        // Create "Hello" thing
        let hello_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"hello");
        let mut fields = graph::map();
        fields.insert(canon::NAME, Value::Text(String::from("Hello")));
        graph::fiat(Some(hello_id), canon::WINDOW, fields);

        // Create "World" thing
        let world_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"world");
        let mut fields = graph::map();
        fields.insert(canon::NAME, Value::Text(String::from("World")));
        graph::fiat(Some(world_id), canon::WINDOW, fields);

        // Link them
        graph::that(hello_id, "NEXT", world_id, 0);

        // --- Graph Smoke Logic ---
        let smoke_node_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, NODE_NAME.as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::NAME, Value::Text(String::from(NODE_NAME)));
        fields.insert(canon::STATUS, Value::Text(String::from("created")));

        let smoke_node = graph::fiat(Some(smoke_node_id), SMOKE_KIND, fields);

        let smoke_watch = ctx.watch_graph(ThingFilter {
            kind: Some(SMOKE_KIND),
            id: None,
        });

        DemoApp {
            window,
            key_count: 0,
            key_log: VecDeque::new(),
            smoke_node,
            smoke_watch,
            smoke_seen_events: false,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = &ev {
            if *watch == self.smoke_watch && thing.id == self.smoke_node {
                self.smoke_seen_events = true;
            }
        }

        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::KEY_PRESSED {
                self.key_count += 1;
            }
            if thing.kind == canon::KEY_EVENT {
                self.record_key_event(&thing);
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        // --- Graph Smoke Logic ---
        if !self.smoke_seen_events && tick % 8 == 0 {
            let mut props = graph::map();
            props.insert(canon::STATUS, Value::Text(String::from("ticking")));
            let _ = graph::set_props(graph::GraphPropsRequest {
                node: self.smoke_node,
                props,
            });
        }

        ctx.clear_window(&self.window);

        ctx.draw_text(
            &self.window,
            format_args!("Keys pressed: {}\n", self.key_count),
        );
        ctx.draw_text(
            &self.window,
            format_args!("Recent key events (most recent last):\n"),
        );
        for entry in self.key_log.iter() {
            ctx.draw_text(&self.window, format_args!("{}\n", entry));
        }
    }
}

impl DemoApp {
    fn record_key_event(&mut self, thing: &graph::GraphThing) {
        let scancode = thing
            .fields
            .get(&canon::SCANCODE)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u8;
        let down = thing
            .fields
            .get(&canon::DOWN)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let key_text = thing
            .fields
            .get(&canon::TEXT)
            .and_then(|v| v.as_text())
            .map(|s| s.to_string());
        let key_symbol = thing
            .fields
            .get(&canon::KEY)
            .and_then(|v| v.as_symbol())
            .map(|s| s.raw());

        let mut line = format!(
            "0x{:02X} {}",
            scancode,
            if down { "down " } else { "up   " }
        );

        if let Some(text) = key_text {
            line.push_str(&format!(" {}", text));
        } else if let Some(sym) = key_symbol {
            line.push_str(&format!(" sym 0x{:04X}", sym));
        } else {
            line.push_str(" (unlabeled)");
        }

        self.key_log.push_back(line);
        while self.key_log.len() > MAX_KEY_LOG {
            self.key_log.pop_front();
        }
    }
}

fn create_demo_image_data() -> alloc::vec::Vec<u8> {
    const TILE: u32 = 16;
    let mut data = vec![0u8; (TILE * TILE * 4) as usize];
    
    for y in 0..TILE {
        for x in 0..TILE {
            let (r, g, b) = demo_tile_color(x, y);
            let idx = ((y * TILE + x) * 4) as usize;
            data[idx] = b;
            data[idx + 1] = g;
            data[idx + 2] = r;
            data[idx + 3] = 0xFF;
        }
    }
    data
}

fn demo_tile_color(x: u32, y: u32) -> (u8, u8, u8) {
    const DEMO_PEACH: (u8, u8, u8) = (0xFF, 0xE4, 0xC4);
    const DEMO_PEACH_DARK: (u8, u8, u8) = (0xF9, 0xC2, 0x9A);
    const DEMO_LAVENDER: (u8, u8, u8) = (0xC9, 0xB8, 0xFF);
    const DEMO_LAVENDER_D: (u8, u8, u8) = (0xA4, 0x8B, 0xE8);

    let bx = (x / 4) % 2;
    let by = (y / 4) % 2;

    match (bx, by) {
        (0, 0) => DEMO_PEACH,
        (1, 0) => DEMO_PEACH_DARK,
        (0, 1) => DEMO_LAVENDER,
        (1, 1) => DEMO_LAVENDER_D,
        _ => DEMO_PEACH,
    }
}
