#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use userland::widget_abi::{
    GraphHandle, InputEvent, Rect, SharedFramebuffer, WidgetAbi, WidgetContext, WidgetEvent,
    WidgetEventRx,
};
use userland::{app_main, canon, graph, App, AppContext, AppEvent, ThingFilter, Value};
use widget_launcher_entry::LauncherEntryWidget;
use widget_scrollbar_thumb::ScrollbarThumbWidget;

enum WidgetState {
    Launcher(<LauncherEntryWidget as WidgetAbi>::State),
    Scrollbar(<ScrollbarThumbWidget as WidgetAbi>::State),
}

struct WidgetHost {
    widget_state: Option<WidgetState>,
    framebuffer: Vec<u8>,
    width: u32,
    height: u32,
    widget_id: Option<userland::uuid::Uuid>,
    prev_down: bool,
}

impl App for WidgetHost {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: None,
        });

        WidgetHost {
            widget_state: None,
            framebuffer: Vec::new(),
            width: 0,
            height: 0,
            widget_id: None,
            prev_down: false,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                if thing.kind == canon::WIDGET {
                    // Initialization
                    if self.widget_state.is_none() {
                        self.widget_id = Some(thing.id);
                        let width = 200; // Should read from thing?
                        let height = 200; // Should read from thing?
                        self.width = width;
                        self.height = height;
                        self.framebuffer = vec![0; (width * height * 4) as usize];

                        let context = WidgetContext {
                            widget_id: thing.id,
                            graph: GraphHandle,
                            events: WidgetEventRx,
                            framebuffer: SharedFramebuffer {
                                width,
                                height,
                                stride: width * 4,
                            },
                        };

                        // Check widget kind
                        let kind_sym = canon::cc('W', 'K');
                        let is_scrollbar = match thing.fields.get(&kind_sym) {
                            Some(Value::Text(s)) => s == "scrollbar_thumb",
                            _ => false,
                        };

                        if is_scrollbar {
                            self.widget_state =
                                Some(WidgetState::Scrollbar(ScrollbarThumbWidget::init(&context)));
                        } else {
                            self.widget_state =
                                Some(WidgetState::Launcher(LauncherEntryWidget::init(&context)));
                        }
                    }

                    // Input Handling
                    if Some(thing.id) == self.widget_id {
                        if let Some(state) = &mut self.widget_state {
                            let x = thing
                                .fields
                                .get(&canon::MOUSE_X)
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;
                            let y = thing
                                .fields
                                .get(&canon::MOUSE_Y)
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;
                            let down = thing
                                .fields
                                .get(&canon::MOUSE_DOWN)
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            let event = if down && !self.prev_down {
                                Some(WidgetEvent::Input(InputEvent::MouseDown {
                                    x,
                                    y,
                                    button: 1,
                                }))
                            } else if !down && self.prev_down {
                                Some(WidgetEvent::Input(InputEvent::MouseUp { x, y, button: 1 }))
                            } else if down || self.prev_down {
                                // Dragging or moving?
                                Some(WidgetEvent::Input(InputEvent::MouseMove { x, y }))
                            } else {
                                None
                            };

                            self.prev_down = down;

                            if let Some(e) = event {
                                match state {
                                    WidgetState::Launcher(s) => {
                                        LauncherEntryWidget::handle_event(s, e)
                                    }
                                    WidgetState::Scrollbar(s) => {
                                        ScrollbarThumbWidget::handle_event(s, e)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(state) = &self.widget_state {
            if let Some(widget_id) = self.widget_id {
                // Draw
                let rect = Rect {
                    x: 0,
                    y: 0,
                    width: self.width,
                    height: self.height,
                };

                match state {
                    WidgetState::Launcher(s) => {
                        LauncherEntryWidget::draw(s, &mut self.framebuffer, rect)
                    }
                    WidgetState::Scrollbar(s) => {
                        ScrollbarThumbWidget::draw(s, &mut self.framebuffer, rect)
                    }
                }

                // Publish
                let mut updates = graph::map();
                updates.insert(canon::BITMAP, Value::Bytes(self.framebuffer.clone()));
                updates.insert(canon::WIDTH, Value::U64(self.width as u64));
                updates.insert(canon::HEIGHT, Value::U64(self.height as u64));
                graph::fiat(Some(widget_id), canon::WIDGET, updates);
            }
        }
    }
}

userland::app_main!(WidgetHost);
