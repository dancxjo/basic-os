use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use x86_64::instructions::interrupts;

use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::input::{KEYBOARD_BUFFER, KEYBOARD_HEAD, process_scancode};
use crate::interrupts::init_interrupts;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::{bootstrap_step, ps2};

pub struct System {
    framebuffer: Rc<RefCell<Framebuffer>>,
    gui: Rc<RefCell<GUI>>,
    clock: Rc<RefCell<Clock>>,
    screen: Rc<RefCell<Screen>>,
}

impl System {
    pub fn new() -> Self {
        info!("Ready? Set? Go!");

        bootstrap_step!("GDT", {
            init_gdt();
        });

        bootstrap_step!("stack recursion tests", {
            let depth = test_stack_recursion(10);
            assert_eq!(depth, 10);
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

        bootstrap_step!("heap basic test", {
            test_heap_basic();
        });

        bootstrap_step!("IDT", {
            init_idt();
        });

        bootstrap_step!("interrupts", {
            init_interrupts();
        });

        bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
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
        Self {
            framebuffer,
            gui,
            clock,
            screen,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();
        let mut last_head = 0;

        loop {
            x86_64::instructions::hlt();

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

fn test_stack_recursion(depth: usize) -> usize {
    if depth == 0 {
        0
    } else {
        1 + test_stack_recursion(depth - 1)
    }
}

pub fn test_heap_basic() {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec::Vec;

    info!("Testing heap allocation...");

    // Test 1: Box allocation
    let heap_box = Box::new(42);
    assert_eq!(*heap_box, 42);
    info!("Box allocation OK.");

    // Test 2: Vec allocation
    let mut heap_vec = Vec::new();
    for i in 0..10 {
        heap_vec.push(i);
    }
    assert_eq!(heap_vec.len(), 10);
    assert_eq!(heap_vec[3], 3);
    info!("Vec allocation OK.");

    // Test 3: Rc allocation
    let heap_rc = Rc::new(9001);
    assert_eq!(*heap_rc, 9001);
    info!("Rc allocation OK.");

    drop(heap_box);
    drop(heap_vec);
    drop(heap_rc);

    info!("Heap test completed successfully!");
}
