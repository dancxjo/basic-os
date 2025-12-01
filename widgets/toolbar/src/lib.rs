#![no_std]

extern crate alloc;

use userland::widget_abi::*;

pub struct ToolbarWidget;

pub struct State;

impl WidgetAbi for ToolbarWidget {
    type State = State;

    fn init(_ctx: &WidgetContext) -> Self::State {
        State
    }

    fn draw(_state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Draw a dark bar
        let color: u32 = 0xFF_20_20_20; // Dark gray

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

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {}

    fn teardown(_state: Self::State) {}
}
