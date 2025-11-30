use alloc::string::{String, ToString};
use userland::prelude::*;
use userland::{canon, graph, simple_uuid, AppEvent, ThingFilter};
use uuid::Uuid;

const DOCUMENT_NAME: &str = "Advent_Notes";

pub struct TextEditor {
    window: WindowHandle,
    document_id: Uuid,
    #[allow(dead_code)]
    view_id: Uuid,
    content: String,
    dirty: bool,
    #[allow(dead_code)]
    watch_id: WatchId,
    ctrl_down: bool,
}

struct Document {
    #[allow(dead_code)]
    id: Uuid,
}

impl userland::Thingable for Document {
    fn kind() -> &'static str {
        "DOCUMENT"
    }
    fn load(thing: &graph::GraphThing) -> Option<Self> {
        Some(Document { id: thing.id })
    }
}

impl App for TextEditor {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        // 1. Find or Create Document
        let doc_uuid = simple_uuid(DOCUMENT_NAME.as_bytes());
        let existing_doc = graph::load_thing::<Document>(doc_uuid);

        if existing_doc.is_none() {
            let mut fields = graph::map();
            fields.insert(canon::NAME, Value::Text(DOCUMENT_NAME.to_string()));
            fields.insert(canon::MIME, Value::Text("text/plain".to_string()));
            fields.insert(canon::ENCODING, Value::Text("utf-8".to_string()));
            fields.insert(canon::DIRTY, Value::Bool(false));
            fields.insert(canon::LENGTH, Value::U64(14)); // "Hello, ThingOS!\n".len()
            graph::fiat(Some(doc_uuid), canon::DOCUMENT, fields);
        }

        // 2. Create View
        let view_uuid = simple_uuid(b"Advent_Notes_View");
        let mut view_fields = graph::map();
        view_fields.insert(canon::KIND, Value::Text("text".to_string()));
        view_fields.insert(canon::MODE, Value::Symbol(canon::EDIT));
        graph::fiat(Some(view_uuid), canon::VIEW, view_fields);

        // Link View -> Document
        graph::that(view_uuid, canon::OF, doc_uuid, 0);

        // 3. Create Window
        let window = ctx.create_window("Advent_Notes — Editor");

        // Link Window -> View
        graph::that(window.window_id(), canon::SHOWS, view_uuid, 0);

        // 4. Subscribe to input events
        let watch_id = ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        TextEditor {
            window,
            document_id: doc_uuid,
            view_id: view_uuid,
            content: "Hello, ThingOS!\n".to_string(),
            dirty: false,
            watch_id,
            ctrl_down: false,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::KEY_EVENT {
                let down = thing
                    .fields
                    .get(&canon::DOWN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if let Some(Value::Symbol(sym)) = thing.fields.get(&canon::KEY) {
                    if *sym == canon::cc('C', 'T') {
                        self.ctrl_down = down;
                    }

                    if down {
                        if *sym == canon::cc('E', 'N') {
                            self.handle_input("\n");
                        } else if *sym == canon::cc('B', 'S') {
                            self.handle_input("\x08");
                        }
                    }
                }

                if down {
                    if let Some(Value::Text(text)) = thing.fields.get(&canon::TEXT) {
                        // Check for Ctrl+S
                        if self.ctrl_down && (text == "s" || text == "S") {
                            self.mark_dirty(false);
                        } else {
                            self.handle_input(text);
                        }
                    }
                }
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        ctx.clear_window(&self.window);

        // Header
        let dirty_marker = if self.dirty { "[dirty]" } else { "" };
        ctx.draw_text(
            &self.window,
            format_args!("Advent_Notes (edit) {}\n", dirty_marker),
        );
        ctx.draw_text(&self.window, format_args!("------------------------\n"));

        // Content
        ctx.draw_text(&self.window, format_args!("{}", self.content));
    }
}

impl TextEditor {
    fn handle_input(&mut self, key: &str) {
        match key {
            "\n" | "\r" => {
                self.content.push('\n');
                self.mark_dirty(true);
            }
            "\x08" => {
                // Backspace
                if self.content.pop().is_some() {
                    self.mark_dirty(true);
                }
            }
            k if k.len() == 1 => {
                // Basic printable characters
                self.content.push_str(k);
                self.mark_dirty(true);
            }
            _ => {}
        }
    }

    fn mark_dirty(&mut self, dirty: bool) {
        if self.dirty != dirty {
            self.dirty = dirty;
            // Update graph
            let mut fields = graph::map();
            fields.insert(canon::DIRTY, Value::Bool(dirty));
            if dirty {
                fields.insert(canon::LENGTH, Value::U64(self.content.len() as u64));
            }
            graph::set_props(graph::GraphPropsRequest {
                node: self.document_id,
                props: fields,
            });
        }
    }
}
