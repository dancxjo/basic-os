#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryInto;
use thing_abi::GraphPropsGetRequest;
use userland::graph::{get_props, map, set_props, GraphPropsRequest};
use userland::widget_abi::*;
use userland::{canon, Symbol, Value};

const ICON_HOME: &[u8] = include_bytes!("../../button/icons/home.bmp");
const ICON_MENU: &[u8] = include_bytes!("../../button/icons/menu.bmp");
const ICON_SETTINGS: &[u8] = include_bytes!("../../button/icons/settings.bmp");
const ICON_CLOSE: &[u8] = include_bytes!("../../button/icons/close.bmp");

pub struct ListboxDefaultWidget;

pub struct ItemState {
    label: String,
    value: String,
    icon_bmp: Option<&'static [u8]>,
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
                    let icon_name = match item_thing.fields.get(&canon::ICON_NAME) {
                        Some(Value::Text(t)) => t.clone(),
                        _ => String::new(),
                    };
                    let icon_bmp = match icon_name.as_str() {
                        "home" => Some(ICON_HOME),
                        "menu" => Some(ICON_MENU),
                        "settings" => Some(ICON_SETTINGS),
                        "close" => Some(ICON_CLOSE),
                        _ => None,
                    };
                    items.push(ItemState {
                        label,
                        value,
                        icon_bmp,
                    });
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

        let item_height = 36;
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

            let mut text_x = item_rect.x + 4;
            if let Some(bmp) = item.icon_bmp {
                draw_bmp(fb, item_rect, 4, (item_height as i32 - 32) / 2, bmp);
                text_x += 36;
            }

            draw_text(
                fb,
                rect,
                text_x,
                item_rect.y + (item_height as i32 - 16) / 2,
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

fn draw_bmp(fb: &mut [u8], rect: Rect, x: i32, y: i32, bmp: &[u8]) {
    if bmp.len() < 54 {
        return;
    }
    let width = i32::from_le_bytes(bmp[18..22].try_into().unwrap_or([0; 4]));
    let height = i32::from_le_bytes(bmp[22..26].try_into().unwrap_or([0; 4]));
    let data_offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap_or([0; 4])) as usize;

    if width <= 0 || height == 0 || data_offset >= bmp.len() {
        return;
    }

    let is_top_down = height < 0;
    let height = height.abs();

    // Assuming 32bpp for now as per my conversion
    // But I should check bit count at offset 28
    let bpp = u16::from_le_bytes(bmp[28..30].try_into().unwrap_or([0; 2]));
    if bpp != 32 {
        return;
    } // Only support 32bpp for these icons

    for row in 0..height {
        for col in 0..width {
            let src_row = if is_top_down { row } else { height - 1 - row };
            let src_idx = data_offset + (src_row as usize * width as usize + col as usize) * 4;

            if src_idx + 4 > bmp.len() {
                continue;
            }

            // BMP is BGRA usually
            let b = bmp[src_idx];
            let g = bmp[src_idx + 1];
            let r = bmp[src_idx + 2];
            let a = bmp[src_idx + 3];

            if a == 0 {
                continue;
            } // Fully transparent

            let dst_x = rect.x + x + col;
            let dst_y = rect.y + y + row;

            if dst_x < rect.x
                || dst_x >= rect.x + rect.width as i32
                || dst_y < rect.y
                || dst_y >= rect.y + rect.height as i32
            {
                continue;
            }

            let dst_offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
            if dst_offset + 4 <= fb.len() {
                // Simple alpha blending
                // dst = src * alpha + dst * (1 - alpha)
                let inv_a = 255 - a;
                let dst_r = fb[dst_offset];
                let dst_g = fb[dst_offset + 1];
                let dst_b = fb[dst_offset + 2];

                fb[dst_offset] = ((r as u16 * a as u16 + dst_r as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 1] =
                    ((g as u16 * a as u16 + dst_g as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 2] =
                    ((b as u16 * a as u16 + dst_b as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 3] = 255; // Opaque alpha for framebuffer
            }
        }
    }
}
