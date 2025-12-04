#![no_std]

extern crate alloc;

use alloc::string::String;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;

use userland::widget_abi::*;
use userland::{canon, Symbol, Value};

pub struct RadioButtonWidget;

const SELECTED: Symbol = canon::canon(b'S', b'E', b'L');

#[derive(Clone, Debug)]
pub struct State {
    pub label: String,
    pub selected: bool,
    pub pressed: bool,
    pub hovered: bool,
}

impl WidgetAbi for RadioButtonWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let mut label = String::from("Radio");
        let mut selected = false;

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::TEXT, SELECTED],
        };

        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                label = t.clone();
            }
            if let Some(Value::Bool(b)) = props.get(&SELECTED) {
                selected = *b;
            }
        }

        State {
            label,
            selected,
            pressed: false,
            hovered: false,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Colors
        let bg_color: u32 = 0xFF_C0_C0_C0;
        let border_light: u32 = 0xFF_FF_FF_FF;
        let border_shadow: u32 = 0xFF_40_40_40;
        let _border_black: u32 = 0xFF_00_00_00;
        let text_color: u32 = 0xFF_00_00_00;
        let check_bg: u32 = 0xFF_FF_FF_FF;
        let dot_color: u32 = 0xFF_00_00_00;

        // Fill background
        for i in 0..fb.len() / 4 {
            fb[i * 4] = (bg_color >> 16) as u8;
            fb[i * 4 + 1] = (bg_color >> 8) as u8;
            fb[i * 4 + 2] = bg_color as u8;
            fb[i * 4 + 3] = (bg_color >> 24) as u8;
        }

        // Draw Radio Circle (12x12 approx)
        let box_size = 12;
        let box_x = 2;
        let box_y = (rect.height as i32 - box_size) / 2;
        let center_x = box_x + box_size / 2;
        let center_y = box_y + box_size / 2;
        let radius = box_size / 2;

        // Draw circle background and border
        for y in 0..box_size {
            for x in 0..box_size {
                let px = box_x + x;
                let py = box_y + y;

                let dx = x - box_size / 2;
                let dy = y - box_size / 2;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= radius * radius {
                    let offset = ((py as usize * rect.width as usize) + px as usize) * 4;

                    // Border logic (simple)
                    if dist_sq >= (radius - 1) * (radius - 1) {
                        // Border
                        // Top/Left shadow
                        if dx < 0 || dy < 0 {
                            fb[offset] = (border_shadow >> 16) as u8;
                            fb[offset + 1] = (border_shadow >> 8) as u8;
                            fb[offset + 2] = border_shadow as u8;
                            fb[offset + 3] = (border_shadow >> 24) as u8;
                        } else {
                            fb[offset] = (border_light >> 16) as u8;
                            fb[offset + 1] = (border_light >> 8) as u8;
                            fb[offset + 2] = border_light as u8;
                            fb[offset + 3] = (border_light >> 24) as u8;
                        }
                    } else {
                        // Background
                        fb[offset] = (check_bg >> 16) as u8;
                        fb[offset + 1] = (check_bg >> 8) as u8;
                        fb[offset + 2] = check_bg as u8;
                        fb[offset + 3] = (check_bg >> 24) as u8;
                    }
                }
            }
        }

        // Draw dot if selected
        if state.selected {
            let dot_radius = 2;
            for y in -dot_radius..=dot_radius {
                for x in -dot_radius..=dot_radius {
                    if x * x + y * y <= dot_radius * dot_radius {
                        let px = center_x + x;
                        let py = center_y + y;
                        let offset = ((py as usize * rect.width as usize) + px as usize) * 4;
                        fb[offset] = (dot_color >> 16) as u8;
                        fb[offset + 1] = (dot_color >> 8) as u8;
                        fb[offset + 2] = dot_color as u8;
                        fb[offset + 3] = (dot_color >> 24) as u8;
                    }
                }
            }
        }

        // Draw Label
        let mut x_off = box_x + box_size + 4;
        let y_off = (rect.height as i32 - 16) / 2; // Centered text

        for c in state.label.chars() {
            if let Some(glyph) = unifont::get_glyph(c) {
                let glyph_width = 8; // Fixed width for now
                if x_off + glyph_width > rect.width as i32 {
                    break;
                }

                for y in 0..16 {
                    for x in 0..8 {
                        if glyph.get_pixel(x, y) {
                            let px = x_off + x as i32;
                            let py = y_off + y as i32;
                            if px >= 0
                                && px < rect.width as i32
                                && py >= 0
                                && py < rect.height as i32
                            {
                                let offset =
                                    ((py as usize * rect.width as usize) + px as usize) * 4;
                                fb[offset] = (text_color >> 16) as u8;
                                fb[offset + 1] = (text_color >> 8) as u8;
                                fb[offset + 2] = text_color as u8;
                                fb[offset + 3] = (text_color >> 24) as u8;
                            }
                        }
                    }
                }
                x_off += glyph_width;
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
                    state.selected = !state.selected; // Toggle for now
                    state.pressed = false;
                }
            }
            WidgetEvent::Input(InputEvent::MouseMove { .. }) => {
                state.hovered = true;
            }
            _ => {}
        }
    }

    fn teardown(_state: Self::State) {}
}
