use alloc::collections::{BTreeMap, VecDeque};
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
    windows: BTreeMap<Uuid, WindowHandle>,
    key_count: usize,
    key_log: VecDeque<String>,
}

impl App for DemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        // Define the layout regions
        struct DemoRegion {
            name: &'static str,
            kind: &'static str,
            title: &'static str,
            x: u64,
            y: u64,
            width: u64,
            height: u64,
        }

        let regions = vec![
            DemoRegion {
                name: "top_bar",
                kind: "top_status_bar",
                title: "Top Bar",
                x: 0,
                y: 0,
                width: 1024,
                height: 40,
            },
            DemoRegion {
                name: "left_rail",
                kind: "listbox_default", // Placeholder for launcher rail
                title: "Launcher",
                x: 0,
                y: 50,
                width: 80,
                height: 700,
            },
            DemoRegion {
                name: "main_content",
                kind: "image", // Placeholder for text editor
                title: "Main Content",
                x: 100,
                y: 60,
                width: 600,
                height: 500,
            },
            DemoRegion {
                name: "mini_graph",
                kind: "graph_mini_viewer",
                title: "Graph View",
                x: 720,
                y: 60,
                width: 280,
                height: 200,
            },
            DemoRegion {
                name: "inspector",
                kind: "thing_inspector",
                title: "Inspector",
                x: 720,
                y: 280,
                width: 280,
                height: 300,
            },
            DemoRegion {
                name: "status_widget",
                kind: "status_widget",
                title: "Status",
                x: 720,
                y: 600,
                width: 280,
                height: 100,
            },
        ];

        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        let mut windows = BTreeMap::new();

        for region in regions {
            // Create Window
            let window_fields = userland::graph::Window {
                id: Uuid::nil(),
                title: region.title.to_string(),
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                z: 0,
                visible: true,
                target: None,
                active: false,
                // Use a regular window so geometry is respected; root windows are full-screen.
                is_root: false,
                mode_index: Some(1), // F2
                window_rect: None,
                gap: None,
                flex_direction: None,
                justify_content: None,
                align_items: None,
            };
            let window = ctx.create_window_with(window_fields);

            // Create Widget
            let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, region.name.as_bytes());
            let mut fields = graph::map();
            fields.insert(canon::cc('W', 'K'), Value::Text(String::from(region.kind)));
            fields.insert(canon::WIDTH, Value::U64(region.width));
            fields.insert(canon::HEIGHT, Value::U64(region.height));
            fields.insert(canon::PARENT, Value::Uuid(window.window_id()));

            // Add specific fields for placeholders if needed
            if region.kind == "image" {
                let bmp_data = create_demo_image_data();
                fields.insert(canon::cc('I', 'D'), Value::Bytes(bmp_data));
            }

            graph::fiat(Some(widget_id), canon::WIDGET, fields);
            graph::grant_capability(widget_host_bundle, widget_id, "CAN_READ");
            graph::that(window.window_id(), "contains", widget_id, 0);

            windows.insert(window.window_id(), window);
        }

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_PRESSED),
            id: None,
        });
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        // Return initial state
        DemoApp {
            windows,
            key_count: 0,
            key_log: VecDeque::new(),
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::KEY_PRESSED {
                self.key_count += 1;
            }
            if thing.kind == canon::KEY_EVENT {
                self.record_key_event(&thing);
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        // No-op for now
    }
}

impl DemoApp {
    fn record_key_event(&mut self, thing: &graph::GraphThing) {
        let key_text = thing
            .fields
            .get(&canon::TEXT)
            .and_then(|v| v.as_text())
            .map(|s| s.to_string());

        if let Some(text) = key_text {
            if self.key_log.len() >= MAX_KEY_LOG {
                self.key_log.pop_front();
            }
            self.key_log.push_back(text);
        }
    }
}

fn create_demo_image_data() -> alloc::vec::Vec<u8> {
    let mut data = vec![0; 16 * 16 * 4];
    for i in 0..16 * 16 {
        data[i * 4] = 255;
        data[i * 4 + 1] = 0;
        data[i * 4 + 2] = 0;
        data[i * 4 + 3] = 255;
    }
    data
}
