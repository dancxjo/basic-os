use core::cell::RefCell;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gui::GUI;
use crate::idt::init_fault_handlers;
use crate::mouse::Mouse;
use crate::paging::{BootFrameAllocator, init_paging};
use crate::screen::Screen;
use crate::{kthread, println, serial};

use alloc::rc::Rc;
use log::info;
use x86_64::structures::paging::frame;

static KEYBOARD_COUNT: AtomicUsize = AtomicUsize::new(0);
static MOUSE_COUNT: AtomicUsize = AtomicUsize::new(0);
static GUI_COUNT: AtomicUsize = AtomicUsize::new(0);
static FRAMEBUFFER_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct ThingOS {
    clock: Rc<RefCell<Clock>>,
    framebuffer: Rc<RefCell<Framebuffer>>,
    gui: Rc<RefCell<GUI>>,
    mouse: Rc<RefCell<Mouse>>,
    screen: Rc<RefCell<Screen>>,
}

impl ThingOS {
    pub fn new() -> Self {
        info!("Bootstrapping ThingOS...");

        // 1. Set up GDT first (so CPU segment registers are sane)
        info!("Initializing GDT...");
        crate::gdt::init_gdt();
        info!("GDT initialized.");

        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        info!("Acquired TSS.");
        init_fault_handlers(); // <==== Early fault handlers ONLY
        info!("Fault handlers initialized.");

        // 🛑 PAUSE: Paging is being set up *before* double fault stack is mapped!
        // This will cause problems if paging touches unmapped stack memory during early faults.
        // You should map the double fault stack *before* setting up paging entirely.

        let frame_allocator = BootFrameAllocator::init();
        info!("Acquired frame allocator.");

        // 📌 INIT PAGING: This is okay, but you need to be careful that your `init_paging`
        // does not itself call `Cr3::write()` too early if your double fault stack is not yet ready.
        info!("Initializing paging...");
        let mut mapper = unsafe { init_paging(frame_allocator) };
        info!("Initialized paging.");
        #[allow(unconditional_panic)]
        let fail = 1 / 0;
        loop {}

        // ✅ Correct: initialize the double fault stack (map it now!)
        info!("Initializing double-fault stack...");
        // crate::idt::init_double_fault_stack(tss, &mut mapper, frame_allocator);
        info!("Double-fault stack initialized.");

        // ✅ Load the IDT
        info!("Initializing IDT...");
        // crate::idt::init_idt();
        info!("IDT initialized.");

        // 🛑 DANGER: Enabling interrupts **before** initializing APIC timer!
        // You cannot safely enable interrupts yet!
        // APIC could fire and CPU has no fully safe timer IRQ handler yet!

        // ❗ MOVE THIS LATER ❗
        // info!("Enabling interrupts...");
        // x86_64::instructions::interrupts::enable(); // <- move this AFTER interrupts::init_interrupts

        // ✅ Now initialize interrupts fully (APIC, LAPIC, etc.)
        crate::interrupts::init_interrupts();
        log::info!("Interrupts initialized (APIC mode).");

        // ✅ NOW enable interrupts
        info!("Enabling interrupts...");
        x86_64::instructions::interrupts::enable();
        info!("GDT and IDT initialized; interrupts enabled.");

        // ✅ Now initialize drivers
        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));
        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Rc::new(RefCell::new(Clock::new(hpet, rtc)));
        let mouse = Rc::new(RefCell::new(Mouse::new(&framebuffer.borrow())));
        let screen = Rc::new(RefCell::new(Screen::new(&framebuffer.borrow())));
        let gui = Rc::new(RefCell::new(GUI::new(&framebuffer.borrow())));

        info!("ThingOS initialized.");

        Self {
            clock,
            framebuffer,
            gui,
            mouse,
            screen,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        // Spawn kernel threads
        kthread::spawn(keyboard_thread);
        kthread::spawn(mouse_thread);
        kthread::spawn(gui_thread);
        kthread::spawn(framebuffer_thread);

        // Forever loop (idle thread)
        loop {
            serial_println!(
                "Idle: keyboard={} mouse={} gui={} framebuffer={} ",
                KEYBOARD_COUNT.load(Ordering::Relaxed),
                MOUSE_COUNT.load(Ordering::Relaxed),
                GUI_COUNT.load(Ordering::Relaxed),
                FRAMEBUFFER_COUNT.load(Ordering::Relaxed)
            );
            for _ in 0..50_000_000 {
                core::hint::spin_loop();
            }
        }
    }
}

extern "C" fn keyboard_thread() {
    loop {
        KEYBOARD_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn mouse_thread() {
    loop {
        MOUSE_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn gui_thread() {
    loop {
        GUI_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn framebuffer_thread() {
    loop {
        FRAMEBUFFER_COUNT.fetch_add(1, Ordering::Relaxed);
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}
