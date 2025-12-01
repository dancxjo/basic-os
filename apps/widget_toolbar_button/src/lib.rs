#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;

use userland::widget_abi::*;
use userland::{canon, Value};

pub struct ToolbarButtonWidget;

pub struct State {
    label: String,
    target: String,
    pressed: bool,
    icon_char: char,
}

impl WidgetAbi for ToolbarButtonWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        // Read properties from graph
        let mut label = String::from("Btn");
        let mut target = String::new();
        let mut icon_name = String::new();

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::TEXT, canon::TARGET, canon::ICON_NAME],
        };

        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                label = t.clone();
            }
            if let Some(Value::Text(t)) = props.get(&canon::TARGET) {
                target = t.clone();
            }
            if let Some(Value::Text(t)) = props.get(&canon::ICON_NAME) {
                icon_name = t.clone();
            }
        }

        let icon_char = match icon_name.as_str() {
            "clouds" => '\u{2601}', // Cloud
            "text" => '\u{1F4DD}',  // Memo
            "graph" => '\u{1F4CA}', // Bar Chart
            _ => '?',
        };

        State {
            label,
            target,
            pressed: false,
            icon_char,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Draw button background
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

        // Draw icon
        draw_char(
            fb,
            rect,
            4,
            (rect.height as i32 - 16) / 2,
            state.icon_char,
            0xFF_FF_FF_FF,
        );

        // Draw label
        let mut x = 24;
        let y = (rect.height as i32 - 16) / 2;
        for c in state.label.chars() {
            draw_char(fb, rect, x, y, c, 0xFF_FF_FF_FF);
            x += 8; // Assuming 8px width for most chars, unifont is 8 or 16
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

fn draw_char(fb: &mut [u8], rect: Rect, x: i32, y: i32, c: char, color: u32) {
    if let Some(glyph) = unifont::get_glyph(c) {
        let glyph_width = glyph.get_width() as i32;
        for row in 0..16 {
            let dst_y = rect.y + y + row;
            if dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
                continue;
            }
            for col in 0..glyph_width {
                let dst_x = rect.x + x + col;
                if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 {
                    continue;
                }
                if glyph.get_pixel(col as usize, row as usize) {
                    let offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
                    if offset + 4 <= fb.len() {
                        fb[offset] = (color >> 16) as u8;
                        fb[offset + 1] = (color >> 8) as u8;
                        fb[offset + 2] = color as u8;
                        fb[offset + 3] = (color >> 24) as u8;
                    }
                }
            }
        }
    }
}
