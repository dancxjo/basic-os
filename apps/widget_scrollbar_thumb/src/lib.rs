#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::cell::RefCell;
use core::cmp::{max, min};
use thing_abi::ThingId;
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
        }
        metrics.track_height = rect.height as i32;

        // 2. Calculate thumb geometry
        let (thumb_y, thumb_height) = calculate_thumb_geometry(*metrics);

        // 3. Draw the track (background)
        // SCROLLBAR_TRACK_COLOR: 0xffE2E6F0
        let track_color = (0xE2, 0xE6, 0xF0, 0xFF); // B G R A
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
        // SCROLLBAR_THUMB_COLOR: 0xff7C8BAB
        // Highlight: 0xffF5F7FB
        let thumb_color = if state.pressed {
            (0xF5, 0xF7, 0xFB, 0xFF)
        } else {
            (0x7C, 0x8B, 0xAB, 0xFF)
        };

        fill_rect(
            fb,
            rect.width as usize,
            0,
            thumb_y,
            rect.width as i32,
            thumb_height,
            thumb_color,
        );
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { y, .. }) => {
                let metrics = state.metrics.borrow();
                let (thumb_y, thumb_height) = calculate_thumb_geometry(*metrics);

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
                    let content_height = metrics.content_height;
                    let viewport_height = metrics.viewport_height;

                    let max_scroll = max(0, content_height - viewport_height);
                    if max_scroll <= 0 {
                        return;
                    }

                    let ratio = track_height as f32 / max(1, content_height) as f32;
                    let mut thumb_height = ((ratio * track_height as f32) + 0.5) as i32;
                    thumb_height = clamp(thumb_height, min(32, track_height), track_height);

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

fn calculate_thumb_geometry(metrics: Metrics) -> (i32, i32) {
    let viewport_height = metrics.viewport_height;
    let content_height = metrics.content_height;
    let track_height = metrics.track_height;
    let scroll_y = metrics.scroll_y;

    if content_height <= 0 || viewport_height <= 0 || track_height <= 0 {
        return (0, 0);
    }

    let max_scroll = max(0, content_height - viewport_height);
    let clamped_scroll = clamp(scroll_y, 0, max_scroll);

    let ratio = track_height as f32 / max(1, content_height) as f32;
    let mut thumb_height = ((ratio * track_height as f32) + 0.5) as i32;
    thumb_height = clamp(thumb_height, min(32, track_height), track_height);

    let max_thumb_offset = track_height - thumb_height;

    let thumb_offset = if max_scroll > 0 {
        let scroll_ratio = clamped_scroll as f32 / max_scroll as f32;
        ((scroll_ratio * max_thumb_offset as f32) + 0.5) as i32
    } else {
        0
    };

    (thumb_offset, thumb_height)
}

fn clamp(v: i32, min_val: i32, max_val: i32) -> i32 {
    max(min_val, min(v, max_val))
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
