#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;
use userland::widget_abi::*;
use userland::{canon, Value};

pub struct ImageWidget;

pub struct State {
    image_data: Vec<u8>,
    width: u32,
    height: u32,
}

impl WidgetAbi for ImageWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let mut image_data = Vec::new();
        let mut width = 0;
        let mut height = 0;

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::cc('I', 'D'), canon::WIDTH, canon::HEIGHT],
        };

        if let Some(props) = get_props(req) {
            if let Some(Value::Bytes(data)) = props.get(&canon::cc('I', 'D')) {
                image_data = data.clone();
            }
            if let Some(Value::U64(w)) = props.get(&canon::WIDTH) {
                width = *w as u32;
            }
            if let Some(Value::U64(h)) = props.get(&canon::HEIGHT) {
                height = *h as u32;
            }
        }

        State {
            image_data,
            width,
            height,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        if state.image_data.is_empty() {
            // Fill with placeholder color (Red)
            let color: u32 = 0xFF_00_00_FF;
            for i in 0..fb.len() / 4 {
                fb[i * 4] = (color >> 16) as u8;
                fb[i * 4 + 1] = (color >> 8) as u8;
                fb[i * 4 + 2] = color as u8;
                fb[i * 4 + 3] = (color >> 24) as u8;
            }
            return;
        }

        let img_stride = state.width * 4;
        let fb_stride = rect.width * 4;

        let copy_width = if state.width < rect.width {
            state.width
        } else {
            rect.width
        };
        let copy_height = if state.height < rect.height {
            state.height
        } else {
            rect.height
        };

        for y in 0..copy_height {
            let src_offset = (y * img_stride) as usize;
            let dst_offset = (y * fb_stride) as usize;

            if src_offset + (copy_width * 4) as usize <= state.image_data.len()
                && dst_offset + (copy_width * 4) as usize <= fb.len()
            {
                fb[dst_offset..dst_offset + (copy_width * 4) as usize].copy_from_slice(
                    &state.image_data[src_offset..src_offset + (copy_width * 4) as usize],
                );
            }
        }
    }

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {}
    fn teardown(_state: Self::State) {}
}
