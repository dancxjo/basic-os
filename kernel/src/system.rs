use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, bootstrap_graph};
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::input::{KEYBOARD_BUFFER, KEYBOARD_HEAD, process_scancode};
use crate::interrupts::init_interrupts;
use crate::mouse::Mouse;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::tasks::SCHEDULER;
use crate::{bootstrap_step, ps2, serial_println};
use alloc::rc::Rc;
use core::cell::RefCell;
use core::sync::atomic::Ordering;
use core::sync::atomic::{AtomicU8, Ordering as AtomicOrdering};
use log::info;
use x86_64::instructions::interrupts;

pub struct System {
    mouse: Rc<RefCell<Mouse>>,
    framebuffer: Rc<RefCell<Framebuffer>>,
    clock: Rc<RefCell<Clock>>,
    screen: Rc<RefCell<Screen>>,
    gui: Rc<RefCell<GUI>>,
    graph: Rc<RefCell<Graph>>,
}

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

        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let mouse = bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
            Rc::new(RefCell::new(Mouse::new(&framebuffer.borrow())))
        });

        bootstrap_step!("tasks", {
            let mut scheduler = SCHEDULER.lock();
            scheduler.spawn(keyboard_thread, 0, &mut mapper, &mut frame_allocator);
            scheduler.spawn(mouse_thread, 1, &mut mapper, &mut frame_allocator);
        });

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Rc::new(RefCell::new(Clock::new(hpet, rtc)));
        let screen = Rc::new(RefCell::new(Screen::new(&framebuffer.borrow())));
        let gui = Rc::new(RefCell::new(GUI::new(&framebuffer.borrow())));
        info!("ThingOS initialized.");
        Self {
            mouse: mouse.clone(),
            framebuffer: framebuffer.clone(),
            clock: clock.clone(),
            screen: screen.clone(),
            gui: gui.clone(),
            graph: graph.clone(),
        }
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
// Mouse packet parsing state

static MOUSE_PACKET: [AtomicU8; 3] = [AtomicU8::new(0), AtomicU8::new(0), AtomicU8::new(0)];
static MOUSE_PACKET_INDEX: AtomicU8 = AtomicU8::new(0);

pub fn process_mouse_packet(byte: u8) {
    let idx = MOUSE_PACKET_INDEX.load(AtomicOrdering::Relaxed) as usize;
    MOUSE_PACKET[idx].store(byte, AtomicOrdering::Relaxed);

    let next_idx = idx + 1;
    if next_idx >= 3 {
        // Parse the 3-byte packet
        let b0 = MOUSE_PACKET[0].load(AtomicOrdering::Relaxed);
        let b1 = MOUSE_PACKET[1].load(AtomicOrdering::Relaxed);
        let b2 = MOUSE_PACKET[2].load(AtomicOrdering::Relaxed);

        // Buttons
        let left = b0 & 0x1 != 0;
        let right = b0 & 0x2 != 0;
        let middle = b0 & 0x4 != 0;

        // Movement (with sign extension)
        let x = if b0 & 0x10 != 0 {
            (b1 as i8) as i32
        } else {
            b1 as i32
        };
        let y = if b0 & 0x20 != 0 {
            (b2 as i8) as i32
        } else {
            b2 as i32
        };

        serial_println!(
            "Mouse packet: buttons: L={} M={} R={}, x={}, y={}",
            left,
            middle,
            right,
            x,
            y
        );

        // TODO: Call your mouse event handler here, e.g. update cursor position

        MOUSE_PACKET_INDEX.store(0, AtomicOrdering::Relaxed);
    } else {
        MOUSE_PACKET_INDEX.store(next_idx as u8, AtomicOrdering::Relaxed);
    }
}

#[unsafe(no_mangle)]
extern "C" fn mouse_thread() {
    let mut last_head = 0;
    loop {
        let head = crate::input::MOUSE_HEAD.load(Ordering::Relaxed);
        if head != last_head {
            let buf = crate::input::MOUSE_PACKET_BUFFER.lock();

            for i in last_head..head {
                let index = i % 256;
                let byte = buf[index];
                process_mouse_packet(byte);
            }
            last_head = head;
        }
    }
}
