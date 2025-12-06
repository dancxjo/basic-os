#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::cell::RefCell;
use core::cmp::{max, min};
use thing_abi::ThingId;
use userland::colors::{
    SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_HILIGHT, SCROLLBAR_THUMB_SHADOW, SCROLLBAR_TRACK_COLOR,
};
use userland::{
    canon,
    graph::{self},
    widget_abi::{InputEvent, Rect, WidgetAbi, WidgetContext, WidgetEvent},
    AbiRequest, AbiResponse, GraphThing, Value,
};

pub struct ScrollbarThumbWidget;

#[derive(Clone, Copy, Debug, Default)]
struct Metrics {
    viewport_height: i32,
    content_height: i32,
    scroll_y: i32,
    track_height: i32,
    max_scroll: i32,
    thumb_offset: i32,
    thumb_height: i32,
}

pub struct State {
    pressed: bool,
    drag_start_mouse_y: i32,
    drag_start_scroll: i32,
    metrics: RefCell<Metrics>,
    widget_id: ThingId,
}

fn get_thing(id: ThingId) -> Option<GraphThing> {
    let response = userland::runtime().call(AbiRequest::Get { id });
    match response {
        AbiResponse::Get { thing } => thing,
        AbiResponse::Fiat { thing } => Some(thing),
        _ => None,
    }
}

impl WidgetAbi for ScrollbarThumbWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        State {
            pressed: false,
            drag_start_mouse_y: 0,
            drag_start_scroll: 0,
            metrics: RefCell::new(Metrics::default()),
            widget_id: ctx.widget_id,
        }
    }

    fn teardown(_state: Self::State) {}

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // 1. Read properties from the widget node
        let mut metrics = state.metrics.borrow_mut();

        if let Some(thing) = get_thing(state.widget_id) {
            if let Some(v) = thing
                .fields
                .get(&canon::VIEWPORT_HEIGHT)
                .and_then(|v| v.as_i64())
            {
                metrics.viewport_height = v as i32;
            }
            if let Some(v) = thing
                .fields
                .get(&canon::CONTENT_HEIGHT)
                .and_then(|v| v.as_i64())
            {
                metrics.content_height = v as i32;
            }
            if let Some(v) = thing.fields.get(&canon::SCROLL_Y).and_then(|v| v.as_i64()) {
                metrics.scroll_y = v as i32;
            }
            if let Some(v) = thing
                .fields
                .get(&canon::MAX_SCROLL)
                .and_then(|v| v.as_i64())
            {
                metrics.max_scroll = v as i32;
            }
            if let Some(v) = thing
                .fields
                .get(&canon::THUMB_OFFSET)
                .and_then(|v| v.as_i64())
            {
                metrics.thumb_offset = v as i32;
            }
            if let Some(v) = thing
                .fields
                .get(&canon::THUMB_HEIGHT)
                .and_then(|v| v.as_i64())
            {
                metrics.thumb_height = v as i32;
            }
        }
        metrics.track_height = rect.height as i32;

        // 2. Calculate thumb geometry
        let (thumb_y, thumb_height) = (metrics.thumb_offset, metrics.thumb_height);

        // 3. Draw the track (background)
        let track_color = argb_to_bgra(SCROLLBAR_TRACK_COLOR);
        fill_rect(
            fb,
            rect.width as usize,
            0,
            0,
            rect.width as i32,
            rect.height as i32,
            track_color,
        );

        // 4. Draw the thumb
        let thumb_color = if state.pressed {
            SCROLLBAR_THUMB_HILIGHT
        } else {
            SCROLLBAR_THUMB_COLOR
        };

        let thumb_bgra = argb_to_bgra(thumb_color);

        fill_rect(
            fb,
            rect.width as usize,
            0,
            thumb_y,
            rect.width as i32,
            thumb_height,
            thumb_bgra,
        );

        if thumb_height > 1 {
            fill_rect(
                fb,
                rect.width as usize,
                0,
                thumb_y,
                rect.width as i32,
                1,
                argb_to_bgra(SCROLLBAR_THUMB_HILIGHT),
            );
            fill_rect(
                fb,
                rect.width as usize,
                0,
                thumb_y + thumb_height - 1,
                rect.width as i32,
                1,
                argb_to_bgra(SCROLLBAR_THUMB_SHADOW),
            );
        }
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { y, .. }) => {
                let metrics = state.metrics.borrow();
                let (thumb_y, thumb_height) = (metrics.thumb_offset, metrics.thumb_height);

                if y >= thumb_y && y < thumb_y + thumb_height {
                    state.pressed = true;
                    state.drag_start_mouse_y = y;
                    state.drag_start_scroll = metrics.scroll_y;
                }
            }
            WidgetEvent::Input(InputEvent::MouseUp { .. }) => {
                state.pressed = false;
            }
            WidgetEvent::Input(InputEvent::MouseMove { y, .. }) => {
                if state.pressed {
                    let metrics = state.metrics.borrow();
                    let dy = y - state.drag_start_mouse_y;

                    let track_height = metrics.track_height;
                    let max_scroll = metrics.max_scroll;

                    if max_scroll <= 0 {
                        return;
                    }

                    let thumb_height = metrics.thumb_height;
                    let max_thumb_offset = track_height - thumb_height;
                    if max_thumb_offset <= 0 {
                        return;
                    }

                    let delta_scroll =
                        (dy as f32 * max_scroll as f32 / max_thumb_offset as f32) as i32;
                    let new_scroll = clamp(state.drag_start_scroll + delta_scroll, 0, max_scroll);

                    let mut updates = graph::map();
                    updates.insert(canon::SCROLL_Y, Value::I64(new_scroll as i64));
                    graph::fiat(Some(state.widget_id), canon::WIDGET, updates);
                }
            }
            _ => {}
        }
    }
}

fn clamp(v: i32, min_val: i32, max_val: i32) -> i32 {
    max(min_val, min(v, max_val))
}

fn argb_to_bgra(color: u32) -> (u8, u8, u8, u8) {
    let a = (color >> 24) as u8;
    let r = (color >> 16) as u8;
    let g = (color >> 8) as u8;
    let b = color as u8;
    (b, g, r, a)
}

fn fill_rect(
    fb: &mut [u8],
    stride_pixels: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: (u8, u8, u8, u8),
) {
    let (b, g, r, a) = color;
    let fb_len = fb.len();

    for row in y..(y + h) {
        if row < 0 {
            continue;
        }
        for col in x..(x + w) {
            if col < 0 || col >= stride_pixels as i32 {
                continue;
            }

            let idx = (row as usize * stride_pixels + col as usize) * 4;
            if idx + 3 < fb_len {
                fb[idx] = b;
                fb[idx + 1] = g;
                fb[idx + 2] = r;
                fb[idx + 3] = a;
            }
        }
    }
}
