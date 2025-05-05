use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use x86_64::instructions::interrupts;

use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::bootstrap_step;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::interrupts::init_interrupts;
use crate::mouse::Mouse;
use crate::panic::halt;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::tasks::{SCHEDULER, Scheduler};

static KEYBOARD_COUNT: AtomicUsize = AtomicUsize::new(0);
static MOUSE_COUNT: AtomicUsize = AtomicUsize::new(0);
static GUI_COUNT: AtomicUsize = AtomicUsize::new(0);
static FRAMEBUFFER_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct System {
    framebuffer: Rc<RefCell<Framebuffer>>,
    gui: Rc<RefCell<GUI>>,
    mouse: Rc<RefCell<Mouse>>,
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
            interrupts::disable();
        });

        bootstrap_step!("tasks", {
            let mut scheduler = SCHEDULER.lock();
            scheduler.spawn(keyboard_thread, 0, &mut mapper, &mut frame_allocator);
            scheduler.spawn(mouse_thread, 1, &mut mapper, &mut frame_allocator);
            scheduler.spawn(gui_thread, 2, &mut mapper, &mut frame_allocator);
            scheduler.spawn(keyboard_thread, 3, &mut mapper, &mut frame_allocator);
            scheduler.spawn(mouse_thread, 4, &mut mapper, &mut frame_allocator);
            scheduler.spawn(gui_thread, 5, &mut mapper, &mut frame_allocator);
            scheduler.spawn(keyboard_thread, 6, &mut mapper, &mut frame_allocator);
        });

        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();

        // Only create the Clock once
        let clock_boxed = Box::new(Clock::new(hpet, rtc));
        let clock_static: &'static Clock = Box::leak(clock_boxed);
        crate::clock::set_global_clock(clock_static);

        // These may still use Rc if needed for UI wiring
        let mouse = Rc::new(RefCell::new(Mouse::new(&framebuffer.borrow())));
        let screen = Rc::new(RefCell::new(Screen::new(&framebuffer.borrow())));
        let gui = Rc::new(RefCell::new(GUI::new(&framebuffer.borrow())));

        info!("ThingOS initialized.");
        Self {
            framebuffer,
            gui,
            mouse,
            screen,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();
        let scheduler = SCHEDULER.lock();
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
    loop {
        let how_long = KEYBOARD_COUNT.load(Ordering::Relaxed);
        if how_long % 2000 == 0 {
            // serial_println!("<");
        }
        KEYBOARD_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..10000000 {
            unsafe { core::arch::asm!("pause") };
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn mouse_thread() {
    loop {
        let how_long = MOUSE_COUNT.load(Ordering::Relaxed);
        if how_long % 5000 == 0 {
            // serial_println!(">");
        }
        MOUSE_COUNT.fetch_add(1, Ordering::Relaxed);

        for _ in 0..1000000 {
            unsafe { core::arch::asm!("pause") };
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn gui_thread() {
    loop {
        GUI_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..1000000 {
            unsafe { core::arch::asm!("pause") };
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn framebuffer_thread() {
    loop {
        FRAMEBUFFER_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..1000000 {
            unsafe { core::arch::asm!("pause") };
        }
    }
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
