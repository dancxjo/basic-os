use log::info;
use spin::mutex::Mutex;
use x86_64::structures::paging::OffsetPageTable;

use crate::arch::x86_64::gdt::init_gdt;
use crate::arch::x86_64::idt::init_idt;
use crate::arch::x86_64::interrupts::init_interrupts;
use crate::arch::x86_64::ps2;
use crate::arch::x86_64::stack::init_kernel_stack;
use crate::bootloader::get_hhdm_offset;
#[cfg(not(feature = "kernel_multitask"))]
use crate::bootloader::get_module;
use crate::bootstrap_step;
use crate::clock::{Clock, HPET, RTC};
use crate::drivers::framebuffer::{Framebuffer, get_framebuffer_info, register_framebuffer_device};
use crate::drivers::{keyboard, mouse, serial};
use crate::mm::allocator::{BootFrameAllocator, init_heap, init_paging, prime_allocator_sanity};
use crate::system::direct_runtime::{KernelDirectRuntime, syscall_handler};
#[cfg(not(feature = "kernel_multitask"))]
use crate::task::executable::{create_user_page_table, jump_to_user, load_elf};
#[cfg(feature = "kernel_multitask")]
use crate::task::launcher;
use crate::task::runtime;
use alloc::boxed::Box;
use alloc::format;
use alloc::sync::Arc;
use spin::Mutex as SpinMutex;
#[cfg(not(feature = "kernel_multitask"))]
use x86_64::PhysAddr;
#[cfg(not(feature = "kernel_multitask"))]
use x86_64::structures::paging::PhysFrame;

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    pub frame_allocator: SpinMutex<BootFrameAllocator>,
    pub mapper: SpinMutex<OffsetPageTable<'static>>,
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: &'static SpinMutex<Clock>,
    // scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
}

impl System {
    pub fn boot() -> Self {
        info!("Ready? Set? Go!");

        let (mut mapper, mut frame_allocator) = init_kernel_core();
        let framebuffer = init_drivers();
        let _clock = init_clock();
        info!("ThingOS initialized.");
        #[cfg(feature = "kernel_multitask")]
        init_user_tasks(&mut mapper, &mut frame_allocator);

        Self {
            frame_allocator: SpinMutex::new(frame_allocator),
            mapper: SpinMutex::new(mapper),
            framebuffer,
            clock: _clock,
            // scheduler,
        }
    }

    pub fn run() -> ! {
        info!("ThingOS running...");

        // --- Heap Self Test ---
        {
            use alloc::vec::Vec;
            info!("Running heap self-test...");
            let mut v = Vec::with_capacity(1024);
            for i in 0..1024 {
                v.push(i);
            }
            drop(v);

            for _ in 0..100 {
                let b = Box::new(0xDEADBEEF_u64);
                drop(b);
            }
            info!("Heap self-test complete.");
        }
        // ----------------------

        info!("System initialized. Entering main loop...");
        // Enable interrupts only after the full system (including the clock) is ready.
        x86_64::instructions::interrupts::enable();
        run_system();
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    let mut guard = SYSTEM.lock();
    if guard.is_some() {
        panic!("System already initialized");
    }
    *guard = Some(system);
    let guard = Box::leak(Box::new(guard));
    let system_ref = guard.as_ref().unwrap();
    runtime::init_system(runtime::SystemConfig {
        mapper: &system_ref.mapper,
        frame_allocator: &system_ref.frame_allocator,
    });
    System::run()
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn task_entry_trampoline() {
    core::arch::naked_asm!(
        "xor rdi, rdi", // clear
        "xor rsi, rsi",
        "call start_user_task",
        "ud2",
    );
}

fn init_kernel_core() -> (OffsetPageTable<'static>, BootFrameAllocator) {
    let (mapper, frame_allocator) = init_memory_and_heap();
    init_graph_and_syscalls();
    init_interrupts_and_idt();
    (mapper, frame_allocator)
}

fn init_memory_and_heap() -> (OffsetPageTable<'static>, BootFrameAllocator) {
    let mut mapper = bootstrap_step!("paging", {
        let physical_memory_offset = get_hhdm_offset();
        unsafe { init_paging(physical_memory_offset) }
    });
    prime_allocator_sanity(&mut mapper);

    let mut frame_allocator = BootFrameAllocator::new();

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
        crate::arch::x86_64::gdt::debug_dump_gdt();
    });

    bootstrap_step!("IDT", {
        init_idt();
    });

    bootstrap_step!("interrupts", {
        init_interrupts();
    });
}

fn init_drivers() -> Arc<SpinMutex<Framebuffer>> {
    info!("[INFO] Starting driver bring-up (framebuffer -> keyboard -> mouse)...");
    let framebuffer = bootstrap_step!("framebuffer driver", {
        let fb = Arc::new(SpinMutex::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));
        register_framebuffer_device(fb.clone());
        fb
    });
    info!("[INFO] Framebuffer driver initialized; framebuffer mapped for userland.");

    let _mouse = bootstrap_step!("PS/2 devices", {
        ps2::enable_ps2_devices();
    });

    bootstrap_step!("serial", {
        serial::init_serial();
    });

    bootstrap_step!("keyboard driver", {
        keyboard::init();
    });
    info!("[INFO] Keyboard driver initialized (raw scancodes buffered).");

    bootstrap_step!("mouse driver", {
        if let Err(err) = mouse::init() {
            info!("Mouse driver init failed: {}", err);
        }
    });
    info!("[INFO] Mouse driver initialized (raw packets buffered).");

    framebuffer
}

fn init_clock() -> &'static SpinMutex<Clock> {
    let hpet = HPET::new(0xFED00000);
    let rtc = RTC::new();
    let clock = Box::leak(Box::new(SpinMutex::new(Clock::new(hpet, rtc))));
    crate::clock::set_global_clock(clock);
    clock
}

#[cfg(feature = "kernel_multitask")]
fn init_user_tasks(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BootFrameAllocator,
) {
    bootstrap_step!("executable", {
        launcher::init_user_modules();
        let count = launcher::user_module_count();
        if count == 0 {
            info!("No user modules to launch.");
        }
        for _ in 0..count {
            let trampoline: extern "C" fn() =
                unsafe { core::mem::transmute(task_entry_trampoline as unsafe extern "C" fn()) };
            runtime::spawn_kernel_with_allocator(trampoline, mapper, frame_allocator);
        }
    });
}

#[cfg(not(feature = "kernel_multitask"))]
fn run_single_user_module(module_name: &str, description: &str) -> ! {
    use x86_64::instructions::hlt;

    info!(
        "Single-task mode: launching {} ({})",
        module_name, description
    );

    let module_bytes = get_module(module_name)
        .or_else(|| get_module(&format!("/boot/{module_name}")))
        .unwrap_or_else(|| panic!("{} not found", module_name));

    let (l4_phys, loaded) = {
        let runtime_system = runtime::system();
        let mut mapper = runtime_system.mapper().lock();
        let mut frame_allocator = runtime_system.frame_allocator().lock();

        let (l4_table, mut user_mapper) =
            create_user_page_table(&mut *frame_allocator, &mut *mapper, get_hhdm_offset());
        let loaded = load_elf(
            module_bytes,
            l4_table,
            &mut user_mapper,
            &mut *frame_allocator,
        )
        .unwrap_or_else(|_| panic!("Failed to load {} ELF", module_name));
        let l4_phys = PhysFrame::containing_address(PhysAddr::new(
            l4_table as *const _ as u64 - get_hhdm_offset().as_u64(),
        ));

        (l4_phys, loaded)
    };

    info!(
        "{} entry prepared: rip={:#x}, stack_top={:#x}, cr3={:#x}",
        module_name,
        loaded.entry.as_u64(),
        loaded.stack_top.as_u64(),
        l4_phys.start_address().as_u64()
    );

    unsafe {
        jump_to_user(loaded.entry, loaded.stack_top, l4_phys);
    }

    info!("{} returned; halting.", module_name);
    loop {
        hlt();
    }
}

#[cfg(feature = "standalone_compositor")]
fn run_standalone_compositor() -> ! {
    // Ensure framebuffer is set up so init can discover it via syscalls
    let _ = get_framebuffer_info().expect("Framebuffer info not available");

    let runtime = Box::new(KernelDirectRuntime);

    // Safety: We are passing the raw framebuffer pointer to the compositor.
    // The kernel will not touch it anymore in this mode (cooperative single task).
    standalone::run_desktop(runtime, syscall_handler);
}

#[cfg(feature = "kernel_multitask")]
fn run_system() -> ! {
    runtime::start()
}

#[cfg(feature = "standalone_compositor")]
fn run_system() -> ! {
    run_standalone_compositor()
}

#[cfg(all(
    not(feature = "kernel_multitask"),
    not(feature = "standalone_compositor")
))]
fn run_system() -> ! {
    panic!("No run mode selected (kernel_multitask or standalone_compositor)");
}
