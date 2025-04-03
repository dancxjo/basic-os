#![no_std]
#![no_main]
extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

mod font8x16;
use font8x16::*;

fn draw_char(
    fb: *mut u32,
    pitch: usize,
    screen_width: usize,
    screen_height: usize,
    x: usize,
    y: usize,
    ch: u8,
    color: u32,
) {
    use crate::font8x16::{CHAR_COUNT, FIRST_CHAR, FONT_DATA};

    let index = ch as usize;
    if index < FIRST_CHAR || index >= FIRST_CHAR + CHAR_COUNT {
        return;
    }

    let glyph = &FONT_DATA[index - FIRST_CHAR];

    for row in 0..8 {
        let lo = glyph[row];
        let hi = glyph[row + 8];

        for col in 0..8 {
            let bit_lo = (lo >> (7 - col)) & 1;
            let bit_hi = (hi >> (7 - col)) & 1;

            if bit_lo != 0 {
                let px = x + col;
                let py = y + row;
                if px < screen_width && py < screen_height {
                    let offset = py * (pitch / 4) + px;
                    unsafe {
                        *fb.add(offset) = color;
                    }
                }
            }

            if bit_hi != 0 {
                let px = x + col;
                let py = y + row + 8;
                if px < screen_width && py < screen_height {
                    let offset = py * (pitch / 4) + px;
                    unsafe {
                        *fb.add(offset) = color;
                    }
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());
    init_heap();

    main();
    hcf();
}
