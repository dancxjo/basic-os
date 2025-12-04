#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use userland::uuid::Uuid;
use userland::widget_abi::{
    GraphHandle, InputEvent, Rect, SharedFramebuffer, WidgetAbi, WidgetContext, WidgetEvent,
    WidgetEventRx,
};
use userland::{canon, graph, App, AppContext, AppEvent, ThingFilter, Value};
use widget_button::ButtonWidget;
use widget_image::ImageWidget;
use widget_launcher_entry::ThingWidget;
use widget_listbox_default::ListboxDefaultWidget;
use widget_notification_dialog::NotificationDialog;
use widget_notification_toast::NotificationToast;
use widget_scrollbar_thumb::ScrollbarThumbWidget;
use widget_toolbar::ToolbarWidget;

enum WidgetState {
    Thing(<ThingWidget as WidgetAbi>::State),
    Scrollbar(<ScrollbarThumbWidget as WidgetAbi>::State),
    Toolbar(<ToolbarWidget as WidgetAbi>::State),
    Button(<ButtonWidget as WidgetAbi>::State),
    ListboxDefault(<ListboxDefaultWidget as WidgetAbi>::State),
    NotificationToast(<NotificationToast as WidgetAbi>::State),
    NotificationDialog(<NotificationDialog as WidgetAbi>::State),
    Image(<ImageWidget as WidgetAbi>::State),
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
    focused_widget: Option<Uuid>,
}

impl App for WidgetHost {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: None,
        });

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::NOTIFICATION),
            id: None,
        });

        WidgetHost {
            widgets: BTreeMap::new(),
            focused_widget: None,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                if thing.kind == canon::NOTIFICATION {
                    let ack = match thing.fields.get(&canon::ACK) {
                        Some(Value::Bool(b)) => *b,
                        _ => false,
                    };

                    if !ack {
                        let scope = match thing.fields.get(&canon::SCOPE) {
                            Some(Value::Text(s)) => s.as_str(),
                            _ => "local",
                        };

                        let widget_kind = if scope == "global" {
                            "notification_dialog"
                        } else {
                            "notification_toast"
                        };

                        let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, thing.id.as_bytes());

                        let mut fields = graph::map();
                        fields.insert(canon::cc('W', 'K'), Value::Text(String::from(widget_kind)));
                        fields.insert(canon::BINDS, Value::Uuid(thing.id));

                        if widget_kind == "notification_dialog" {
                            fields.insert(canon::WIDTH, Value::U64(400));
                            fields.insert(canon::HEIGHT, Value::U64(200));
                        } else {
                            fields.insert(canon::WIDTH, Value::U64(300));
                            fields.insert(canon::HEIGHT, Value::U64(80));
                        }

                        graph::fiat(Some(widget_id), canon::WIDGET, fields);
                    }
                }

                if thing.kind == canon::WIDGET {
                    // Initialization
                    if !self.widgets.contains_key(&thing.id) {
                        let width_prop = thing.fields.get(&canon::WIDTH).and_then(|v| v.as_u64());
                        let height_prop = thing.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64());

                        let mut width = width_prop.unwrap_or(200) as u32;
                        let mut height = height_prop.unwrap_or(200) as u32;

                        // Check widget kind
                        let kind_sym = canon::cc('W', 'K');
                        let widget_kind_str = match thing.fields.get(&kind_sym) {
                            Some(Value::Text(s)) => Some(s.as_str()),
                            _ => None,
                        };

                        if matches!(widget_kind_str, Some("toolbar_button") | Some("button"))
                            && height_prop.is_none()
                        {
                            height = 32;
                        }

                        let mut context = WidgetContext {
                            widget_id: thing.id,
                            graph: GraphHandle,
                            events: WidgetEventRx,
                            framebuffer: SharedFramebuffer {
                                width,
                                height,
                                stride: width * 4,
                            },
                        };

                        let state = match widget_kind_str {
                            Some("scrollbar_thumb") => {
                                Some(WidgetState::Scrollbar(ScrollbarThumbWidget::init(&context)))
                            }
                            Some("toolbar") => {
                                Some(WidgetState::Toolbar(ToolbarWidget::init(&context)))
                            }
                            Some("toolbar_button") | Some("button") => {
                                let state = ButtonWidget::init(&context);
                                let (measured_w, measured_h) =
                                    ButtonWidget::intrinsic_size(&state, height);
                                let new_width = if width_prop.is_some() {
                                    measured_w.max(width)
                                } else {
                                    measured_w
                                };
                                let new_height = if height_prop.is_some() {
                                    measured_h.max(height)
                                } else {
                                    measured_h
                                };

                                if new_width != width || new_height != height {
                                    width = new_width;
                                    height = new_height;
                                    context.framebuffer = SharedFramebuffer {
                                        width,
                                        height,
                                        stride: width * 4,
                                    };

                                    let mut updates = graph::map();
                                    updates.insert(canon::WIDTH, Value::U64(width as u64));
                                    updates.insert(canon::HEIGHT, Value::U64(height as u64));
                                    graph::fiat(Some(thing.id), canon::WIDGET, updates);
                                }

                                Some(WidgetState::Button(state))
                            }
                            Some("listbox_default") | Some("list") => Some(
                                WidgetState::ListboxDefault(ListboxDefaultWidget::init(&context)),
                            ),
                            Some("notification_toast") => Some(WidgetState::NotificationToast(
                                NotificationToast::init(&context),
                            )),
                            Some("notification_dialog") => Some(WidgetState::NotificationDialog(
                                NotificationDialog::init(&context),
                            )),
                            Some("image") => Some(WidgetState::Image(ImageWidget::init(&context))),
                            Some("thing_tile") => {
                                Some(WidgetState::Thing(ThingWidget::init(&context)))
                            }
                            _ => {
                                // Default to launcher for now if unspecified or unknown
                                Some(WidgetState::Thing(ThingWidget::init(&context)))
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
                            self.focused_widget = Some(thing.id);
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
                                WidgetState::Thing(s) => ThingWidget::handle_event(s, e),
                                WidgetState::Scrollbar(s) => {
                                    ScrollbarThumbWidget::handle_event(s, e)
                                }
                                WidgetState::Toolbar(s) => ToolbarWidget::handle_event(s, e),
                                WidgetState::Button(s) => ButtonWidget::handle_event(s, e),
                                WidgetState::ListboxDefault(s) => {
                                    ListboxDefaultWidget::handle_event(s, e)
                                }
                                WidgetState::NotificationToast(s) => {
                                    NotificationToast::handle_event(s, e)
                                }
                                WidgetState::NotificationDialog(s) => {
                                    NotificationDialog::handle_event(s, e)
                                }
                                WidgetState::Image(s) => ImageWidget::handle_event(s, e),
                            }
                        }
                    }
                } else if thing.kind == canon::KEY_EVENT {
                    if let Some(widget_id) = self.focused_widget {
                        if let Some(instance) = self.widgets.get_mut(&widget_id) {
                            let key = thing
                                .fields
                                .get(&canon::KEY)
                                .and_then(|v| v.as_symbol())
                                .map(|s| s.raw())
                                .unwrap_or(0);
                            let down = thing
                                .fields
                                .get(&canon::DOWN)
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            let event = if down {
                                Some(WidgetEvent::Input(InputEvent::KeyDown { key }))
                            } else {
                                Some(WidgetEvent::Input(InputEvent::KeyUp { key }))
                            };

                            if let Some(e) = event {
                                match &mut instance.state {
                                    WidgetState::Thing(s) => ThingWidget::handle_event(s, e),
                                    WidgetState::Scrollbar(s) => {
                                        ScrollbarThumbWidget::handle_event(s, e)
                                    }
                                    WidgetState::Toolbar(s) => ToolbarWidget::handle_event(s, e),
                                    WidgetState::Button(s) => ButtonWidget::handle_event(s, e),
                                    WidgetState::ListboxDefault(s) => {
                                        ListboxDefaultWidget::handle_event(s, e)
                                    }
                                    WidgetState::NotificationToast(s) => {
                                        NotificationToast::handle_event(s, e)
                                    }
                                    WidgetState::NotificationDialog(s) => {
                                        NotificationDialog::handle_event(s, e)
                                    }
                                    WidgetState::Image(s) => ImageWidget::handle_event(s, e),
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
                WidgetState::Thing(s) => ThingWidget::draw(s, &mut instance.framebuffer, rect),
                WidgetState::Scrollbar(s) => {
                    ScrollbarThumbWidget::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::Toolbar(s) => ToolbarWidget::draw(s, &mut instance.framebuffer, rect),
                WidgetState::Button(s) => ButtonWidget::draw(s, &mut instance.framebuffer, rect),
                WidgetState::ListboxDefault(s) => {
                    ListboxDefaultWidget::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::NotificationToast(s) => {
                    NotificationToast::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::NotificationDialog(s) => {
                    NotificationDialog::draw(s, &mut instance.framebuffer, rect)
                }
                WidgetState::Image(s) => ImageWidget::draw(s, &mut instance.framebuffer, rect),
            }

            // Publish
            let mut updates = graph::map();
            updates.insert(canon::BITMAP, Value::Bytes(instance.framebuffer.clone()));
            // Don't update width/height here as we read it from the thing initially.
            graph::fiat(Some(*id), canon::WIDGET, updates);
        }
    }
}
