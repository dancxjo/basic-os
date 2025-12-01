#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use userland::widget_abi::*;
use userland::{canon, graph};
use userland::graph::{Thingable, GraphThing};
use thing_abi::Map;

pub struct ThingWidget;

pub struct State {
    label: String,
}

struct GenericThing {
    fields: Map,
}

impl Thingable for GenericThing {
    fn kind() -> &'static str { "any" }
    fn load(thing: &GraphThing) -> Option<Self> {
        Some(Self { fields: thing.fields.clone() })
    }
}

impl WidgetAbi for ThingWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let label = if let Some(thing) = graph::load_thing::<GenericThing>(ctx.widget_id) {
            thing
                .fields
                .get(&canon::LABEL)
                .or_else(|| thing.fields.get(&canon::TEXT))
                .and_then(|v| match v {
                    userland::Value::Text(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "Thing".to_string())
        } else {
            "Thing".to_string()
        };

        State { label }
    }

    fn draw(_state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Simple drawing: fill with gray
        // TODO: Render text (state.label)
        let color: u32 = 0xFF_44_44_44; // Dark gray

        for y in 0..rect.height {
            for x in 0..rect.width {
                let offset = ((rect.y + y as i32) as usize * rect.width as usize
                    + (rect.x + x as i32) as usize)
                    * 4;
                if offset + 4 <= fb.len() {
                    fb[offset] = (color >> 16) as u8; // B
                    fb[offset + 1] = (color >> 8) as u8; // G
                    fb[offset + 2] = color as u8; // R
                    fb[offset + 3] = (color >> 24) as u8; // A
                }
            }
        }
    }

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {
        // Handle input
    }

    fn teardown(_state: Self::State) {
        // Cleanup
    }
}
