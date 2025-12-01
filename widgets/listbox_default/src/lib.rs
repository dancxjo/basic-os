#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use thing_abi::GraphPropsGetRequest;
use userland::graph::{get_props, map, set_props, GraphPropsRequest};
use userland::widget_abi::*;
use userland::{canon, Symbol, Value};

pub struct ListboxDefaultWidget;

pub struct ItemState {
    label: String,
    value: String,
}

pub struct State {
    items: Vec<ItemState>,
    selected_index: i32,
    focused_index: i32,
    has_focus: bool,
    control_node: userland::uuid::Uuid,
    value_node: Option<userland::uuid::Uuid>,
}

impl WidgetAbi for ListboxDefaultWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let control_node = ctx.widget_id;
        let mut items = Vec::new();
        let mut selected_index = 0;
        let mut value_node = None;

        let req_binds = GraphPropsGetRequest {
            node: control_node,
            keys: alloc::vec![canon::BINDS],
        };
        if let Some(props) = get_props(req_binds) {
            if let Some(Value::Uuid(v_id)) = props.get(&canon::BINDS) {
                value_node = Some(*v_id);
                let req = GraphPropsGetRequest {
                    node: *v_id,
                    keys: alloc::vec![canon::SELECTED_INDEX],
                };
                if let Some(props) = get_props(req) {
                    if let Some(Value::I64(idx)) = props.get(&canon::SELECTED_INDEX) {
                        selected_index = *idx as i32;
                    }
                }
            }
        }

        let item_kind_str = String::from(canon::ITEM);
        let all_items = userland::graph::find_by_kind(&item_kind_str);

        for item_thing in all_items {
            if let Some(Value::Uuid(parent_id)) = item_thing.fields.get(&canon::PARENT) {
                if *parent_id == control_node {
                    let label = match item_thing.fields.get(&canon::ITEM_LABEL) {
                        Some(Value::Text(t)) => t.clone(),
                        _ => String::from("?"),
                    };
                    let value = match item_thing.fields.get(&canon::ITEM_VALUE) {
                        Some(Value::Text(t)) => t.clone(),
                        _ => String::from(""),
                    };
                    items.push(ItemState { label, value });
                }
            }
        }

        State {
            items,
            selected_index,
            focused_index: selected_index,
            has_focus: false,
            control_node,
            value_node,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        fill_rect(fb, rect, 0xFF_22_22_22);

        let item_height = 24;
        let mut y = rect.y;

        for (i, item) in state.items.iter().enumerate() {
            if y + item_height as i32 > rect.y + rect.height as i32 {
                break;
            }

            let item_rect = Rect {
                x: rect.x,
                y: y,
                width: rect.width,
                height: item_height,
            };

            let is_selected = i as i32 == state.selected_index;
            let is_focused = i as i32 == state.focused_index && state.has_focus;

            let bg_color = if is_selected {
                0xFF_44_44_AA
            } else {
                0xFF_33_33_33
            };

            fill_rect(fb, item_rect, bg_color);

            if is_focused {
                draw_rect_outline(fb, item_rect, 0xFF_FF_FF_00);
            }

            draw_text(
                fb,
                rect,
                item_rect.x + 4,
                item_rect.y + 4,
                &item.label,
                0xFF_FF_FF_FF,
            );

            y += item_height as i32;
        }
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::KeyDown { key }) => {
                let key_sym = Symbol::new(key);
                let mut changed = false;

                if key_sym == canon::cc('A', 'U') {
                    // Up
                    if state.focused_index > 0 {
                        state.focused_index -= 1;
                        changed = true;
                    }
                } else if key_sym == canon::cc('A', 'D') {
                    // Down
                    if state.focused_index < (state.items.len() as i32 - 1) {
                        state.focused_index += 1;
                        changed = true;
                    }
                } else if key_sym == canon::cc('H', 'M') {
                    // Home
                    state.focused_index = 0;
                    changed = true;
                } else if key_sym == canon::cc('E', 'D') {
                    // End
                    state.focused_index = (state.items.len() as i32 - 1).max(0);
                    changed = true;
                } else if key_sym == canon::cc('E', 'N') || key_sym == canon::cc(' ', ' ') {
                    // Enter or Space
                    state.selected_index = state.focused_index;
                    update_selection(state);
                    changed = true;
                }

                if changed {
                    // Focus changed
                }
            }
            WidgetEvent::Input(InputEvent::MouseDown { x: _, y, .. }) => {
                let item_height = 24;
                let index = y / item_height;
                if index >= 0 && index < state.items.len() as i32 {
                    state.selected_index = index;
                    state.focused_index = index;
                    state.has_focus = true;
                    update_selection(state);
                }
            }
            _ => {}
        }
    }

    fn teardown(_state: Self::State) {}
}

fn update_selection(state: &State) {
    if let Some(v_node) = state.value_node {
        let mut updates = map();
        updates.insert(
            canon::SELECTED_INDEX,
            Value::I64(state.selected_index as i64),
        );
        set_props(GraphPropsRequest {
            node: v_node,
            props: updates,
        });
    }
}

fn fill_rect(fb: &mut [u8], rect: Rect, color: u32) {
    for y in rect.y..rect.y + rect.height as i32 {
        for x in rect.x..rect.x + rect.width as i32 {
            if x < 0 || y < 0 {
                continue;
            }
            let offset = ((y as usize * rect.width as usize) + x as usize) * 4;
            if offset + 4 <= fb.len() {
                fb[offset] = (color >> 16) as u8;
                fb[offset + 1] = (color >> 8) as u8;
                fb[offset + 2] = color as u8;
                fb[offset + 3] = (color >> 24) as u8;
            }
        }
    }
}

fn draw_rect_outline(fb: &mut [u8], rect: Rect, color: u32) {
    for x in rect.x..rect.x + rect.width as i32 {
        set_pixel(fb, rect.width, x, rect.y, color);
        set_pixel(fb, rect.width, x, rect.y + rect.height as i32 - 1, color);
    }
    for y in rect.y..rect.y + rect.height as i32 {
        set_pixel(fb, rect.width, rect.x, y, color);
        set_pixel(fb, rect.width, rect.x + rect.width as i32 - 1, y, color);
    }
}

fn set_pixel(fb: &mut [u8], stride_width: u32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 {
        return;
    }
    let offset = ((y as usize * stride_width as usize) + x as usize) * 4;
    if offset + 4 <= fb.len() {
        fb[offset] = (color >> 16) as u8;
        fb[offset + 1] = (color >> 8) as u8;
        fb[offset + 2] = color as u8;
        fb[offset + 3] = (color >> 24) as u8;
    }
}

fn draw_text(fb: &mut [u8], rect: Rect, x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for c in text.chars() {
        draw_char(fb, rect, cx, y, c, color);
        cx += 8;
    }
}

fn draw_char(fb: &mut [u8], rect: Rect, x: i32, y: i32, c: char, color: u32) {
    if let Some(glyph) = unifont::get_glyph(c) {
        let glyph_width = glyph.get_width() as i32;
        for row in 0..16 {
            let dst_y = y + row;
            if dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
                continue;
            }
            for col in 0..glyph_width {
                let dst_x = x + col;
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
