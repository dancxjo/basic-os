use crate::allocator::{self, BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::{self, get_hhdm_offset};
use crate::clock::{Clock, HPET, RTC};
use crate::executable::{create_user_page_table, jump_to_user, load_elf};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, bootstrap_graph};
use crate::idt::init_idt;
use crate::interrupts::init_interrupts;
use crate::mouse::Mouse;
use crate::stack::init_kernel_stack;
use crate::{bootstrap_step, ps2};
use alloc::sync::Arc;
use log::info;
use spin::Mutex as SpinMutex;
use spin::Mutex;
use x86_64::PhysAddr;
use x86_64::instructions::{hlt, interrupts};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PhysFrame;

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    mouse: Arc<SpinMutex<Mouse>>,
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: Arc<SpinMutex<Clock>>,
    graph: Arc<SpinMutex<Graph>>,
    keyboard_index: usize,
    mouse_index: usize,
    // scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
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

        bootstrap_step!("syscalls", {
            crate::syscall::init_syscall();
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

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Arc::new(SpinMutex::new(Clock::new(hpet, rtc)));
        info!("ThingOS initialized.");

        bootstrap_step!("executable", {
            let module = bootloader::get_module("boot/hello_from")
                .expect("Module 'boot/hello_from' not found");
            let (new_l4, mut new_mapper) =
                create_user_page_table(frame_allocator, get_hhdm_offset());
            let loaded = load_elf(module, new_l4, &mut new_mapper, frame_allocator)
                .expect("Failed to load ELF");

            let (frame, _) = Cr3::read(); // get current context
            let new_table_frame = PhysFrame::containing_address(PhysAddr::new(
                new_l4 as *const _ as u64 - get_hhdm_offset().as_u64(),
            ));

            unsafe {
                jump_to_user(loaded.entry, loaded.stack_top, new_table_frame);
            }
        });

        Self {
            mouse,
            framebuffer,
            clock,
            graph,
            keyboard_index: 0,
            mouse_index: 0,
            // scheduler,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        info!("System initialized. Entering main loop...");
        interrupts::enable();

        loop {
            hlt();
        }
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}
