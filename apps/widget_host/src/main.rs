#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use userland::widget_abi::{
    GraphHandle, Rect, SharedFramebuffer, WidgetAbi, WidgetContext, WidgetEventRx,
};
use userland::{app_main, canon, graph, App, AppContext, AppEvent, ThingFilter, Value};
use widget_launcher_entry::LauncherEntryWidget;

struct WidgetHost {
    widget_state: Option<<LauncherEntryWidget as WidgetAbi>::State>,
    framebuffer: Vec<u8>,
    width: u32,
    height: u32,
    widget_id: Option<userland::uuid::Uuid>,
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
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                if thing.kind == canon::WIDGET {
                    // Simple logic: pick the first widget we see and drive it
                    if self.widget_state.is_none() {
                        self.widget_id = Some(thing.id);
                        let width = 200;
                        let height = 30;
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

                        self.widget_state = Some(LauncherEntryWidget::init(&context));
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
                LauncherEntryWidget::draw(
                    state,
                    &mut self.framebuffer,
                    Rect {
                        x: 0,
                        y: 0,
                        width: self.width,
                        height: self.height,
                    },
                );

                // Publish
                let mut updates = graph::map();
                updates.insert(canon::BITMAP, Value::Bytes(self.framebuffer.clone()));
                updates.insert(canon::WIDTH, Value::U64(self.width as u64));
                updates.insert(canon::HEIGHT, Value::U64(self.height as u64));
                // We use fiat to update the existing thing
                graph::fiat(Some(widget_id), canon::WIDGET, updates);
            }
        }
    }
}

userland::app_main!(WidgetHost);
