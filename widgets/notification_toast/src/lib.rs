#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use userland::prelude::*;
use userland::{canon, graph, Value};
use userland::widget_abi::{WidgetAbi, WidgetContext, WidgetEvent, InputEvent, Rect};
use uuid::Uuid;

pub struct NotificationToast;

#[derive(Default)]
pub struct State {
    message: String,
    level: String,
    notification_id: Option<Uuid>,
}

// Wrapper to load raw GraphThing
struct RawThing(userland::graph::GraphThing);

impl userland::graph::Thingable for RawThing {
    fn kind() -> &'static str { "ANY" }
    fn load(thing: &userland::graph::GraphThing) -> Option<Self> {
        Some(RawThing(thing.clone()))
    }
}

impl WidgetAbi for NotificationToast {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let mut state = State::default();
        
        // Load the widget node itself to find what it is bound to
        let widget_node: RawThing = match graph::load_thing(ctx.widget_id) {
            Some(t) => t,
            None => return state,
        };

        if let Some(Value::Uuid(target_id)) = widget_node.0.fields.get(&canon::BINDS) {
            state.notification_id = Some(*target_id);
        }

        if let Some(nid) = state.notification_id {
            // Load the notification node
            // We can use RawThing here too as we just want fields
            if let Some(notif) = graph::load_thing::<RawThing>(nid) {
                if let Some(Value::Text(msg)) = notif.0.fields.get(&canon::MESSAGE) {
                    state.message = msg.clone();
                }
                if let Some(Value::Text(lvl)) = notif.0.fields.get(&canon::LEVEL) {
                    state.level = lvl.clone();
                }
            }
        }

        state
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { .. }) => {
                // Dismiss on click
                if let Some(nid) = state.notification_id {
                    let mut updates = graph::map();
                    updates.insert(canon::ACK, Value::Bool(true));
                    graph::fiat(Some(nid), canon::NOTIFICATION, updates);
                }
            }
            _ => {}
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Background color based on level
        let bg_color = match state.level.as_str() {
            "error" => 0xFF_40_00_00,
            "warn" => 0xFF_40_40_00,
            _ => 0xFF_20_20_20,
        };

        let border_color = 0xFF_80_80_80;

        // Fill background
        draw_rect_fill(fb, rect, rect.x, rect.y, rect.width, rect.height, bg_color);
        
        // Draw border
        draw_rect_outline(fb, rect, rect.x, rect.y, rect.width, rect.height, border_color);

        // Draw text
        draw_string(fb, rect, rect.x + 10, rect.y + 10, &state.message, 0xFF_FF_FF_FF);
    }

    fn teardown(_state: Self::State) {}
}

fn draw_rect_fill(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: u32, h: u32, color: u32) {
    for row in 0..h {
        let dst_y = y + row as i32;
        if dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
            continue;
        }
        for col in 0..w {
            let dst_x = x + col as i32;
            if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 {
                continue;
            }
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

fn draw_rect_outline(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: u32, h: u32, color: u32) {
    // Top
    draw_rect_fill(fb, rect, x, y, w, 1, color);
    // Bottom
    draw_rect_fill(fb, rect, x, y + h as i32 - 1, w, 1, color);
    // Left
    draw_rect_fill(fb, rect, x, y, 1, h, color);
    // Right
    draw_rect_fill(fb, rect, x + w as i32 - 1, y, 1, h, color);
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

fn draw_string(fb: &mut [u8], rect: Rect, x: i32, y: i32, s: &str, color: u32) {
    let mut cx = x;
    for c in s.chars() {
        draw_char(fb, rect, cx, y, c, color);
        cx += 8;
    }
}
