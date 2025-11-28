use log::info;
use spin::mutex::Mutex;
use x86_64::structures::paging::OffsetPageTable;

use crate::arch::x86_64::gdt::init_gdt;
use crate::arch::x86_64::idt::init_idt;
use crate::arch::x86_64::interrupts::init_interrupts;
use crate::arch::x86_64::ps2;
use crate::arch::x86_64::stack::init_kernel_stack;
use crate::bootloader::get_hhdm_offset;
use crate::bootstrap_step;
use crate::clock::{Clock, HPET, RTC};
use crate::drivers::framebuffer::{Framebuffer, init_console, register_framebuffer_device};
use crate::drivers::{keyboard, mouse, serial};
use crate::mm::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::task::{launcher, runtime};
use alloc::boxed::Box;
use alloc::sync::Arc;
use spin::Mutex as SpinMutex;

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: &'static SpinMutex<Clock>,
    // scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
}

impl System {
    pub fn boot() -> Self {
        info!("Ready? Set? Go!");

        let (mut _mapper, mut _frame_allocator) = init_memory_and_heap();
        init_graph_and_syscalls();
        init_interrupts_and_idt();
        let framebuffer = init_framebuffer_and_devices();
        let _clock = init_clock();
        info!("ThingOS initialized.");
        init_user_tasks();

        Self {
            framebuffer,
            clock: _clock,
            // scheduler,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        info!("System initialized. Entering main loop...");
        // Enable interrupts only after the full system (including the clock) is ready.
        x86_64::instructions::interrupts::enable();
        runtime::start();
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    let mut guard = SYSTEM.lock();
    *guard = Some(system);
    guard.as_mut().unwrap().run()
}

#[unsafe(no_mangle)]
pub extern "C" fn task_entry_trampoline() {
    unsafe {
        core::arch::asm!(
            "xor rdi, rdi", // clear
            "xor rsi, rsi",
            "call start_user_task",
            options(noreturn)
        );
    }
}

fn init_memory_and_heap() -> (
    &'static mut OffsetPageTable<'static>,
    &'static mut BootFrameAllocator,
) {
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
        // Test allocator immediately after init
        let mut v = alloc::vec::Vec::new();
        v.push(42);
        log::info!("Allocator test in boot: v[0] = {}", v[0]);
    });

    (mapper, frame_allocator)
}

fn init_graph_and_syscalls() {
    bootstrap_step!("graph", {
        crate::graph::init();
    });

    bootstrap_step!("syscalls", {
        crate::arch::x86_64::syscall::init_syscall();
    });
}

fn init_interrupts_and_idt() {
    bootstrap_step!("GDT", {
        init_gdt();
    });

    bootstrap_step!("IDT", {
        init_idt();
    });

    bootstrap_step!("interrupts", {
        init_interrupts();
    });
}

fn init_framebuffer_and_devices() -> Arc<SpinMutex<Framebuffer>> {
    let framebuffer = bootstrap_step!("framebuffer", {
        let fb = Arc::new(SpinMutex::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));
        init_console(fb.clone());
        register_framebuffer_device(fb.clone());
        fb
    });

    bootstrap_step!("framebuffer graph", {
        crate::drivers::framebuffer::publish_framebuffer_node(framebuffer.clone());
    });

    let _mouse = bootstrap_step!("PS/2 devices", {
        ps2::enable_ps2_devices();
    });

    bootstrap_step!("serial", {
        serial::init_serial();
    });

    bootstrap_step!("keyboard driver", {
        keyboard::init();
    });

    bootstrap_step!("mouse driver", {
        if let Err(err) = mouse::init() {
            info!("Mouse driver init failed: {}", err);
        }
    });

    framebuffer
}

fn init_clock() -> &'static SpinMutex<Clock> {
    let hpet = HPET::new(0xFED00000);
    let rtc = RTC::new();
    let clock = Box::leak(Box::new(SpinMutex::new(Clock::new(hpet, rtc))));
    crate::clock::set_global_clock(clock);
    clock
}

fn init_user_tasks() {
    bootstrap_step!("executable", {
        launcher::init_user_modules();
        let count = launcher::user_module_count();
        if count == 0 {
            info!("No user modules to launch.");
        }
        for _ in 0..count {
            runtime::spawn_kernel(launcher::start_user_task);
        }
    });
}
