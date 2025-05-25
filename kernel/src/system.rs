use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::compositor::compositor::{Compositor, Layer};
use crate::compositor::surface::Surface;
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, bootstrap_graph};
use crate::gui::GUI;
use crate::hello_task::create_hello_task;
use crate::idt::init_idt;
use crate::input::{
    KEYBOARD_BUFFER, KEYBOARD_HEAD, MOUSE_HEAD, MOUSE_PACKET_BUFFER, MOUSE_TAIL, process_scancode,
};
use crate::interrupts::init_interrupts;
use crate::mouse::Mouse;
use crate::scheduler::WasmTask;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::{bootstrap_step, ps2, scheduler};
use alloc::format;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU8, Ordering};
use embedded_graphics::prelude::{OriginDimensions, RgbColor};
use embedded_graphics::primitives::Rectangle;
use log::info;
use spin::Mutex as SpinMutex;
use x86_64::instructions::{hlt, interrupts};

use spin::Mutex;

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    mouse: Arc<SpinMutex<Mouse>>,
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: Arc<SpinMutex<Clock>>,
    screen: Arc<SpinMutex<Screen>>,
    gui: Arc<SpinMutex<GUI>>,
    graph: Arc<SpinMutex<Graph>>,
    keyboard_index: usize,
    mouse_index: usize,
    scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
}

impl System {
    pub fn boot() -> Self {
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

        let graph = bootstrap_step!("graph", { Arc::new(SpinMutex::new(bootstrap_graph())) });

        let framebuffer = bootstrap_step!("framebuffer", {
            Arc::new(SpinMutex::new(
                Framebuffer::new().expect("Framebuffer not available"),
            ))
        });

        let mouse = bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
            Arc::new(SpinMutex::new(Mouse::new(&framebuffer.lock())))
        });

        let scheduler = bootstrap_step!("scheduler", {
            Arc::new(SpinMutex::new(crate::scheduler::Scheduler::new()))
        });

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Arc::new(SpinMutex::new(Clock::new(hpet, rtc)));
        let screen = Arc::new(SpinMutex::new(Screen::new(&framebuffer.lock())));
        let gui = Arc::new(SpinMutex::new(GUI::new(&framebuffer.lock())));
        info!("ThingOS initialized.");
        Self {
            mouse,
            framebuffer,
            clock,
            screen,
            gui,
            graph,
            keyboard_index: 0,
            mouse_index: 0,
            scheduler,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();

        let mut scheduler = self.scheduler.lock();
        for i in 0..4 {
            let task = create_hello_task(i).expect("Failed to create hello task");
            scheduler.add_task(task);
        }
        loop {
            scheduler.schedule();
        }
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}
