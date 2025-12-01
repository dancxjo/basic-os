#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use userland::uuid::Uuid;
use userland::widget_abi::{
    GraphHandle, InputEvent, Rect, SharedFramebuffer, WidgetAbi, WidgetContext, WidgetEvent,
    WidgetEventRx,
};
use userland::{canon, graph, App, AppContext, AppEvent, ThingFilter, Value};
use widget_launcher_entry::LauncherEntryWidget;
use widget_scrollbar_thumb::ScrollbarThumbWidget;
use widget_toolbar::ToolbarWidget;
use widget_toolbar_button::ToolbarButtonWidget;

enum WidgetState {
    Launcher(<LauncherEntryWidget as WidgetAbi>::State),
    Scrollbar(<ScrollbarThumbWidget as WidgetAbi>::State),
    Toolbar(<ToolbarWidget as WidgetAbi>::State),
    ToolbarButton(<ToolbarButtonWidget as WidgetAbi>::State),
}

struct WidgetInstance {
    state: WidgetState,
    framebuffer: Vec<u8>,
    width: u32,
    height: u32,
    prev_down: bool,
}

pub struct WidgetHost {
    widgets: BTreeMap<Uuid, WidgetInstance>,
}

impl App for WidgetHost {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: None,
        });

        WidgetHost {
            widgets: BTreeMap::new(),
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                if thing.kind == canon::WIDGET {
                    // Initialization
                    if !self.widgets.contains_key(&thing.id) {
                        let width = thing
                            .fields
                            .get(&canon::WIDTH)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(200) as u32;
                        let height = thing
                            .fields
                            .get(&canon::HEIGHT)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(200) as u32;

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
                        let widget_kind_str = match thing.fields.get(&kind_sym) {
                            Some(Value::Text(s)) => Some(s.as_str()),
                            _ => None,
                        };

                        let state = match widget_kind_str {
                            Some("scrollbar_thumb") => {
                                Some(WidgetState::Scrollbar(ScrollbarThumbWidget::init(&context)))
                            }
                            Some("toolbar") => {
                                Some(WidgetState::Toolbar(ToolbarWidget::init(&context)))
                            }
                            Some("toolbar_button") => Some(WidgetState::ToolbarButton(
                                ToolbarButtonWidget::init(&context),
                            )),
                            _ => {
                                // Default to launcher for now if unspecified or unknown
                                Some(WidgetState::Launcher(LauncherEntryWidget::init(&context)))
                            }
                        };

                        if let Some(s) = state {
                            self.widgets.insert(
                                thing.id,
                                WidgetInstance {
                                    state: s,
                                    framebuffer: vec![0; (width * height * 4) as usize],
                                    width,
                                    height,
                                    prev_down: false,
                                },
                            );
                        }
                    }

                    // Input Handling
                    if let Some(instance) = self.widgets.get_mut(&thing.id) {
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

                        let event = if down && !instance.prev_down {
                            Some(WidgetEvent::Input(InputEvent::MouseDown {
                                x,
                                y,
                                button: 1,
                            }))
                        } else if !down && instance.prev_down {
                            Some(WidgetEvent::Input(InputEvent::MouseUp { x, y, button: 1 }))
                        } else if down || instance.prev_down {
                            Some(WidgetEvent::Input(InputEvent::MouseMove { x, y }))
                        } else {
                            None
                        };

                        instance.prev_down = down;

                        if let Some(e) = event {
                            match &mut instance.state {
                                WidgetState::Launcher(s) => LauncherEntryWidget::handle_event(s, e),
                                WidgetState::Scrollbar(s) => {
                                    ScrollbarThumbWidget::handle_event(s, e)
                                }
                                WidgetState::Toolbar(s) => ToolbarWidget::handle_event(s, e),
                                WidgetState::ToolbarButton(s) => {
                                    ToolbarButtonWidget::handle_event(s, e)
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
        for (id, instance) in self.widgets.iter_mut() {
            // Draw
            let rect = Rect {
                x: 0,
                y: 0,
                width: instance.width,
                height: instance.height,
            };

            match &instance.state {
                WidgetState::Launcher(s) => {
                    LauncherEntryWidget::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::Scrollbar(s) => {
                    ScrollbarThumbWidget::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::Toolbar(s) => ToolbarWidget::draw(s, &mut instance.framebuffer, rect),
                WidgetState::ToolbarButton(s) => {
                    ToolbarButtonWidget::draw(s, &mut instance.framebuffer, rect)
                }
            }

            // Publish
            let mut updates = graph::map();
            updates.insert(canon::BITMAP, Value::Bytes(instance.framebuffer.clone()));
            // Don't update width/height here as we read it from the thing initially.
            graph::fiat(Some(*id), canon::WIDGET, updates);
        }
    }
}
