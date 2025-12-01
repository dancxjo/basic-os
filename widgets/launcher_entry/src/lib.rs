#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use userland::widget_abi::*;

pub struct LauncherEntryWidget;

pub struct State {
    label: String,
}

impl WidgetAbi for LauncherEntryWidget {
    type State = State;

    fn init(_ctx: &WidgetContext) -> Self::State {
        // In a real implementation, we would read the label from the graph using ctx.widget_id
        State {
            label: "Launcher Entry".to_string(),
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Simple drawing: fill with gray, draw text
        // For now, just fill with a color to prove it works
        let color: u32 = 0xFF_44_44_44; // Dark gray

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

    fn handle_event(_state: &mut Self::State, _event: WidgetEvent) {
        // Handle input
    }

    fn teardown(_state: Self::State) {
        // Cleanup
    }
}
