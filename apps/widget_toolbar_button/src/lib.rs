#![no_std]

extern crate alloc;

use alloc::string::String;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;

use userland::widget_abi::*;
use userland::{canon, Value};

pub struct ToolbarButtonWidget;

pub struct State {
    label: String,
    target: String,
    pressed: bool,
}

impl WidgetAbi for ToolbarButtonWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        // Read properties from graph
        let mut label = String::from("Btn");
        let mut target = String::new();

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::TEXT, canon::TARGET],
        };

        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                label = t.clone();
            }
            if let Some(Value::Text(t)) = props.get(&canon::TARGET) {
                target = t.clone();
            }
        }

        State {
            label,
            target,
            pressed: false,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Draw button
        let color: u32 = if state.pressed {
            0xFF_88_88_88
        } else {
            0xFF_55_55_55
        };

        for y in 0..rect.height {
            for x in 0..rect.width {
                let offset = ((rect.y + y as i32) as usize * rect.width as usize
                    + (rect.x + x as i32) as usize)
                    * 4;
                if offset + 4 <= fb.len() {
                    fb[offset] = (color >> 16) as u8;
                    fb[offset + 1] = (color >> 8) as u8;
                    fb[offset + 2] = color as u8;
                    fb[offset + 3] = (color >> 24) as u8;
                }
            }
        }
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { .. }) => {
                state.pressed = true;
            }
            WidgetEvent::Input(InputEvent::MouseUp { .. }) => {
                if state.pressed {
                    state.pressed = false;
                    // Trigger launch
                    if !state.target.is_empty() {
                        // Emit LaunchRequest
                        let mut fields = userland::map();
                        fields.insert(canon::PACKAGE, Value::Text(state.target.clone()));
                        fields.insert(canon::NAME, Value::Text(state.label.clone()));
                        userland::fiat(None, canon::LAUNCH_REQUEST, fields);
                    }
                }
            }
            _ => {}
        }
    }

    fn teardown(_state: Self::State) {}
}
