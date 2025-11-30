use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, Symbol, ThingFilter};
use uuid::Uuid;

const SMOKE_KIND: Symbol = canon::canon(b'G', b'S', b'M');
const NODE_NAME: &str = "graph-smoke-node";
const MAX_KEY_LOG: usize = 24;

pub struct DemoApp {
    window: WindowHandle,
    bmp_data: &'static [u8],
    key_count: usize,
    key_log: VecDeque<String>,
    sent_bitmap: bool,
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

        let bmp_data = include_bytes!("../../../clouds.bmp");

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
            bmp_data,
            key_count: 0,
            key_log: VecDeque::new(),
            sent_bitmap: false,
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

        if !self.sent_bitmap {
            ctx.draw_bitmap(&self.window, self.bmp_data);
            self.sent_bitmap = true;
        }

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
