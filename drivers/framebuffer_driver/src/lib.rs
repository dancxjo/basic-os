#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]
#![allow(unused)]

extern crate alloc;

use alloc::collections::BTreeMap;
use userland::prelude::*;
use userland::{canon, sys};
use uuid::Uuid;

pub struct FramebufferDriver {
    fb_id: Uuid,
    fb_ptr: *mut u32,
    fb_info: sys::FramebufferInfo,
    frame_watch: Option<WatchId>,
}

impl App for FramebufferDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let fb_info = sys::fb_info().expect("Failed to get framebuffer info");
        let fb_ptr = sys::fb_map() as *mut u32;

        let mut fields = BTreeMap::new();
        fields.insert(canon::KIND, Value::Symbol(canon::DISPLAY_FRAMEBUFFER));
        fields.insert(canon::NAME, Value::Text("fb0".into()));
        fields.insert(canon::WIDTH, Value::U64(fb_info.width));
        fields.insert(canon::HEIGHT, Value::U64(fb_info.height));
        fields.insert(canon::PITCH, Value::U64(fb_info.pitch));
        fields.insert(canon::BPP, Value::U64(fb_info.bpp));

        let fb_id = fiat(None, canon::DISPLAY_FRAMEBUFFER, fields);

        // Watch for current frame
        let filter = ThingFilter {
            kind: Some(canon::DISPLAY_FRAME),
            id: None,
        };
        let frame_watch = Some(ctx.watch_graph(filter));

        FramebufferDriver {
            fb_id,
            fb_ptr,
            fb_info,
            frame_watch,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        // No polling needed, handled in on_event
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing: _ } = ev {
            if Some(watch) == self.frame_watch {
                // Blitting is now handled by the compositor directly
            }
        }
    }
}

impl FramebufferDriver {
    fn blit(&mut self, src: *const u32) {
        let len = (self.fb_info.pitch * self.fb_info.height / 4) as usize;
        unsafe {
            core::ptr::copy_nonoverlapping(src, self.fb_ptr, len);
        }
    }
}

app_main!(FramebufferDriver);
