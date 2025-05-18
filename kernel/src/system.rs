use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, Kind, KindMeta, bootstrap_graph};
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::input::{KEYBOARD_BUFFER, KEYBOARD_HEAD, process_scancode};
use crate::interrupts::init_interrupts;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::tasks::SCHEDULER;
use crate::{bootstrap_step, kind_meta, ps2};
use alloc::boxed::Box;
use alloc::fmt;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::sync::atomic::Ordering;
use log::info;
use serde::{Deserialize, Serialize};
use x86_64::instructions::interrupts;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {}
kind_meta!(System, "system");

impl System {
    pub fn new() -> Self {
        info!("Ready? Set? Go!");

        bootstrap_step!("GDT", {
            init_gdt();
        });

        let mut mapper = bootstrap_step!("paging", {
            let physical_memory_offset = get_hhdm_offset();
            unsafe { init_paging(physical_memory_offset) }
        });

        let mut frame_allocator = BootFrameAllocator::init();

        bootstrap_step!("stack", {
            unsafe { init_kernel_stack(&mut mapper, &mut frame_allocator) };
        });

        bootstrap_step!("heap", {
            init_heap(&mut mapper, &mut frame_allocator);
        });

        bootstrap_step!("IDT", {
            init_idt();
        });

        bootstrap_step!("interrupts", {
            init_interrupts();
        });

        let graph = bootstrap_step!("graph", { Rc::new(RefCell::new(bootstrap_graph())) });

        bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
        });

        bootstrap_step!("tasks", {
            let mut scheduler = SCHEDULER.lock();
            scheduler.spawn(keyboard_thread, 0, &mut mapper, &mut frame_allocator);
        });

        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Rc::new(RefCell::new(Clock::new(hpet, rtc)));
        let screen = Rc::new(RefCell::new(Screen::new(&framebuffer.borrow())));
        let gui = Rc::new(RefCell::new(GUI::new(&framebuffer.borrow())));
        info!("ThingOS initialized.");
        Self {}
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        let scheduler = SCHEDULER.lock();

        interrupts::enable();
        scheduler.start_first();
    }
}

#[macro_export]
macro_rules! bootstrap_step {
    ($desc:expr, $block:expr) => {{
        info!("Initializing {}...", $desc);
        let result = $block;
        info!("Init {} complete.\n", $desc);
        result
    }};
}

#[unsafe(no_mangle)]
extern "C" fn keyboard_thread() {
    let mut last_head = 0;

    loop {
        let head = KEYBOARD_HEAD.load(Ordering::Relaxed);
        if head != last_head {
            let buf = KEYBOARD_BUFFER.lock();

            for i in last_head..head {
                let index = i % 256;
                let byte = buf[index];
                process_scancode(byte);
            }
            last_head = head;
        }
    }
}
