use log::info;
use spin::mutex::Mutex;
use x86_64::instructions::hlt;
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable};

use crate::arch::x86_64::gdt::init_gdt;
use crate::arch::x86_64::idt::init_idt;
use crate::arch::x86_64::interrupts::init_interrupts;
use crate::arch::x86_64::ps2;
use crate::arch::x86_64::stack::init_kernel_stack;
use crate::bootloader::{get_hhdm_offset, get_module};
use crate::bootstrap_step;
use crate::clock::{Clock, HPET, RTC};
use crate::drivers::framebuffer::Framebuffer;
use crate::mm::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::task::context::TaskMode;
use crate::task::executable::{create_user_page_table, jump_to_user, load_elf};
use crate::task::scheduler::SCHEDULER;
use alloc::sync::Arc;
use spin::Mutex as SpinMutex;
use x86_64::PhysAddr;
use x86_64::structures::paging::PhysFrame;
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageTableFlags as Flags},
};

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: Arc<SpinMutex<Clock>>,
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
            crate::arch::x86_64::syscall::init_syscall();
        });

        bootstrap_step!("IDT", {
            init_idt();
        });

        bootstrap_step!("interrupts", {
            init_interrupts();
        });

        let _framebuffer_init = bootstrap_step!("framebuffer", {
            Arc::new(SpinMutex::new(
                Framebuffer::new().expect("Framebuffer not available"),
            ))
        });

        let _mouse = bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
        });

        let _framebuffer = Arc::new(Mutex::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let _clock = Arc::new(SpinMutex::new(Clock::new(hpet, rtc)));
        info!("ThingOS initialized.");

        bootstrap_step!("executable", {
            let mut sched = SCHEDULER.lock();
            sched.spawn(
                hello_thread,
                TaskMode::Kernel,
                &mut mapper,
                &mut frame_allocator,
            );
            sched.spawn(
                second_thread,
                TaskMode::Kernel,
                &mut mapper,
                &mut frame_allocator,
            );
            sched.spawn(
                start_user_task,
                TaskMode::Kernel,
                &mut mapper,
                &mut frame_allocator,
            );
        });

        Self {
            framebuffer: _framebuffer,
            clock: _clock,
            keyboard_index: 0,
            mouse_index: 0,
            // scheduler,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        info!("System initialized. Entering main loop...");
        SCHEDULER.lock().start_first();
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}

#[unsafe(no_mangle)]
pub extern "C" fn task_entry_trampoline() {
    unsafe {
        core::arch::asm!(
            "xor rdi, rdi", // clear
            "xor rsi, rsi",
            "call hello_thread",
            options(noreturn)
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hello_thread() {
    loop {
        crate::println!("Hello from kernel task");
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn second_thread() {
    loop {
        crate::println!("Greetings from task two");
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn start_user_task() {
    let module = get_module("boot/hello_from").expect("Module 'boot/hello_from' not found");
    let frame_allocator = BootFrameAllocator::global();
    let (new_l4, mut new_mapper) = create_user_page_table(frame_allocator, get_hhdm_offset());
    let loaded =
        load_elf(module, new_l4, &mut new_mapper, frame_allocator).expect("Failed to load ELF");
    let new_table_frame = PhysFrame::containing_address(PhysAddr::new(
        new_l4 as *const _ as u64 - get_hhdm_offset().as_u64(),
    ));
    unsafe {
        jump_to_user(loaded.entry, loaded.stack_top, new_table_frame);
    }
}

pub const USER_BINARY_LOAD_BASE: u64 = 0x0000_4000_0000_0000; // 256 GiB

pub fn load_elf_executable(
    elf: &[u8],
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
) -> *const u8 {
    // --- Validate ELF header ---
    assert_eq!(&elf[0..4], b"\x7FELF", "Not a valid ELF file");
    let virt_base = USER_BINARY_LOAD_BASE;

    let e_entry = u64::from_le_bytes(elf[0x18..0x20].try_into().unwrap());
    let phoff = u64::from_le_bytes(elf[0x20..0x28].try_into().unwrap()) as usize;
    let phentsize = u16::from_le_bytes(elf[0x36..0x38].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(elf[0x38..0x3a].try_into().unwrap()) as usize;

    for i in 0..phnum {
        let ph = &elf[phoff + i * phentsize..phoff + (i + 1) * phentsize];

        let p_type = u32::from_le_bytes(ph[0x00..0x04].try_into().unwrap());
        const PT_LOAD: u32 = 1;
        if p_type != PT_LOAD {
            continue;
        }

        let p_offset = u64::from_le_bytes(ph[0x08..0x10].try_into().unwrap());
        let p_vaddr = virt_base + u64::from_le_bytes(ph[0x10..0x18].try_into().unwrap());
        let p_filesz = u64::from_le_bytes(ph[0x20..0x28].try_into().unwrap());
        let p_memsz = u64::from_le_bytes(ph[0x28..0x30].try_into().unwrap());

        assert!(
            (p_offset + p_filesz) <= elf.len() as u64,
            "Segment out of ELF file bounds"
        );

        let start = VirtAddr::new(p_vaddr);
        let end = VirtAddr::new(p_vaddr + p_memsz);
        use x86_64::structures::paging::Size4KiB;
        let page_range = Page::<Size4KiB>::range_inclusive(
            Page::<Size4KiB>::containing_address(start.align_down(0x1000u64)),
            Page::<Size4KiB>::containing_address(end.align_up(0x1000u64) - 1u64),
        );

        for page in page_range {
            let frame = frame_allocator.allocate_frame().expect("Out of frames");
            unsafe {
                mapper
                    .map_to(
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                        frame_allocator,
                    )
                    .expect("map_to failed")
                    .flush();
            }
        }

        // --- Copy file contents ---
        if p_filesz > 0 {
            let src = elf
                .get(p_offset as usize..(p_offset + p_filesz) as usize)
                .expect("ELF slice out of bounds");
            if p_vaddr == 0 {
                log::warn!(
                    "Skipping PT_LOAD with null p_vaddr (offset = {:#x})",
                    p_offset
                );
                continue;
            }
            let dst = p_vaddr as *mut u8;
            assert!(dst as usize % 8 == 0, "destination not aligned");
            log::info!(
                "Loading segment: offset={:#x}, vaddr={:#x}, filesz={}, memsz={}",
                p_offset,
                p_vaddr,
                p_filesz,
                p_memsz
            );

            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
            }
        }

        // --- Zero BSS (if any) ---
        if p_memsz > p_filesz {
            let bss_start = (p_vaddr + p_filesz) as *mut u8;
            let bss_len = (p_memsz - p_filesz) as usize;

            unsafe {
                core::ptr::write_bytes(bss_start, 0, bss_len);
            }
        }
    }
    let adjusted_entry = USER_BINARY_LOAD_BASE + (e_entry & 0x0000_ffff_ffff_ffff);
    return adjusted_entry as *const u8;
}
