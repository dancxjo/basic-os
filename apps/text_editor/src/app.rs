use alloc::format;
use alloc::string::{String, ToString};
use core::convert::TryFrom;
use core::sync::atomic::{AtomicU64, Ordering};
use userland::prelude::*;
use userland::{canon, graph, simple_uuid, AppEvent, ThingFilter};
use uuid::Uuid;

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

const DOCUMENT_NAME: &str = "Advent_Notes";
const LINE_HEIGHT: i32 = 16;
const HEADER_LINES: i32 = 2;
const CHAR_WIDTH: i32 = 8;
const CURSOR_FOCUS_NUDGE: i32 = 4;
const CURSOR_VERTICAL_NUDGE: i32 = 4;
const CURSOR_BLINK_PERIOD_TICKS: u64 = 24;

pub struct TextEditor {
    window: WindowHandle,
    document_id: Uuid,
    #[allow(dead_code)]
    view_id: Uuid,
    content: String,
    cursor_index: usize,
    dirty: bool,
    #[allow(dead_code)]
    watch_id: WatchId,
    ctrl_down: bool,
    cursor_hint_dirty: bool,
    cursor_visible: bool,
    cursor_blink_ticks: u64,
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
        // 0. Get Self
        let self_id = userland::sys::get_self();

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

            // Grant CAN_EDIT to self
            graph::grant_capability(self_id, doc_uuid, canon::CAN_EDIT);
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

        let mut editor = TextEditor {
            window,
            document_id: doc_uuid,
            view_id: view_uuid,
            content: "Hello, ThingOS!\n".to_string(),
            cursor_index: "Hello, ThingOS!\n".len(),
            dirty: false,
            watch_id,
            ctrl_down: false,
            cursor_hint_dirty: true,
            cursor_visible: true,
            cursor_blink_ticks: 0,
        };

        editor.flush_cursor_hint();
        editor
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
                            self.create_save_event();
                        } else {
                            self.handle_input(text);
                        }
                    }
                }
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        // Reassert the cursor focus rect every frame so compositor scrollbars keep following
        // the caret, even if the user scrolls manually for a moment.
        self.cursor_hint_dirty = true;
        self.cursor_blink_ticks = self.cursor_blink_ticks.saturating_add(1);
        if self.cursor_blink_ticks >= CURSOR_BLINK_PERIOD_TICKS {
            self.cursor_visible = !self.cursor_visible;
            self.cursor_blink_ticks = 0;
        }
        ctx.clear_window(&self.window);

        // Header
        let dirty_marker = if self.dirty { "[dirty]" } else { "" };
        ctx.draw_text(
            &self.window,
            format_args!("Advent_Notes (edit) {}\n", dirty_marker),
        );
        ctx.draw_text(&self.window, format_args!("------------------------\n"));

        // Content with cursor indicator
        let (head, tail) = self.content.split_at(self.cursor_index);
        let mut display = String::with_capacity(self.content.len() + 1);
        display.push_str(head);
        if self.cursor_visible {
            display.push('|');
        }
        display.push_str(tail);
        ctx.draw_text(&self.window, format_args!("{}", display));

        let (line, column) = self.cursor_line_col();
        ctx.draw_text(
            &self.window,
            format_args!(
                "\n\nCursor: line {} column {} — {}\n",
                line + 1,
                column + 1,
                if self.cursor_visible {
                    "visible"
                } else {
                    "hidden (blinking)"
                }
            ),
        );

        self.flush_cursor_hint();
    }
}

impl TextEditor {
    fn handle_input(&mut self, key: &str) {
        match key {
            "\n" | "\r" => {
                self.insert_text("\n");
            }
            "\x08" => {
                self.delete_prev_char();
            }
            k if k.len() == 1 => {
                self.insert_text(k);
            }
            _ => {}
        }
    }

    fn create_save_event(&self) {
        let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let event_id = Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!("save-event-{}", counter).as_bytes(),
        );
        let mut fields = graph::map();
        fields.insert(canon::NAME, Value::Text("Save Event".into()));
        graph::fiat(Some(event_id), canon::SAVE_EVENT, fields);

        graph::that(event_id, canon::APPLIES_TO, self.document_id, 0);
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

    fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.content.insert_str(self.cursor_index, text);
        self.cursor_index += text.len();
        self.on_content_changed();
    }

    fn delete_prev_char(&mut self) {
        if self.cursor_index == 0 {
            return;
        }
        let prev_idx = self
            .content
            .get(..self.cursor_index)
            .and_then(|prefix| prefix.char_indices().next_back().map(|(idx, _)| idx))
            .unwrap_or(0);
        self.content.drain(prev_idx..self.cursor_index);
        self.cursor_index = prev_idx;
        self.on_content_changed();
    }

    fn on_content_changed(&mut self) {
        self.cursor_hint_dirty = true;
        self.reset_cursor_blink();
        self.mark_dirty(true);
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        let mut line = 0_usize;
        let mut column = 0_usize;
        if let Some(prefix) = self.content.get(..self.cursor_index) {
            for ch in prefix.chars() {
                if ch == '\n' {
                    line += 1;
                    column = 0;
                } else {
                    column += 1;
                }
            }
        }
        (line, column)
    }

    fn cursor_rect(&self) -> (i32, i32, i32, i32) {
        let (line, column) = self.cursor_line_col();
        let line_offset = i32::try_from(line).unwrap_or(i32::MAX);
        let column_offset = i32::try_from(column).unwrap_or(i32::MAX);
        let y = HEADER_LINES
            .saturating_add(line_offset)
            .saturating_mul(LINE_HEIGHT)
            .saturating_sub(CURSOR_VERTICAL_NUDGE)
            .max(0);
        let mut x = column_offset
            .saturating_mul(CHAR_WIDTH)
            .saturating_sub(CURSOR_FOCUS_NUDGE);
        if x < 0 {
            x = 0;
        }
        let width = CHAR_WIDTH
            .saturating_add(CURSOR_FOCUS_NUDGE * 2)
            .max(CHAR_WIDTH);
        let height = LINE_HEIGHT.saturating_add(CURSOR_VERTICAL_NUDGE);
        (x, y, width, height)
    }

    fn flush_cursor_hint(&mut self) {
        if !self.cursor_hint_dirty {
            return;
        }
        let (x, y, width, height) = self.cursor_rect();
        let mut rect = graph::map();
        rect.insert(canon::X, Value::I64(x as i64));
        rect.insert(canon::Y, Value::I64(y as i64));
        rect.insert(canon::WIDTH, Value::I64(width as i64));
        rect.insert(canon::HEIGHT, Value::I64(height as i64));
        let mut props = graph::map();
        props.insert(canon::WINDOW_RECT, Value::Map(rect));
        graph::set_props(graph::GraphPropsRequest {
            node: self.window.window_id(),
            props,
        });
        self.cursor_hint_dirty = false;
    }

    fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.cursor_blink_ticks = 0;
    }
}
