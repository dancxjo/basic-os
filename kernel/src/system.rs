use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::sync::atomic::Ordering;
use log::info;
use x86_64::instructions::{hlt, interrupts};
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable};

use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::{get_hhdm_offset, get_module};
use crate::clock::{CLOCK, Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::input::{KEYBOARD_BUFFER, KEYBOARD_HEAD, process_scancode};
use crate::interrupts::init_interrupts;
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::tasks::SCHEDULER;
use crate::{bootstrap_step, ps2};

pub struct System {
    framebuffer: Rc<RefCell<Framebuffer>>,
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

        bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
        });

        bootstrap_step!("tasks", {
            let mut scheduler = SCHEDULER.lock();
            // scheduler.spawn(keyboard_thread, &mut mapper, &mut frame_allocator);
            scheduler.spawn(task_entry_trampoline, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello2_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello2_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello2_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello_thread, &mut mapper, &mut frame_allocator);
            // scheduler.spawn(hello2_thread, &mut mapper, &mut frame_allocator);

            let module = get_module("boot/hello_from.bin").expect("missing hello_from.bin");
            log::info!("Module found: {} bytes", module.len());
            for (i, b) in module.iter().take(16).enumerate() {
                log::info!("module[{}] = {:#x}", i, b);
            }
            // let load_addr = USER_BINARY_LOAD_BASE + (1 as u64 * 0x200000); // 2 MiB per task
            // let ptr = load_elf_executable(module, &mut mapper, &mut frame_allocator);
            // log::info!("Loaded binary to ptr {:#x}", ptr as u64);
            // log::info!("First byte at {:#x}: {:#x}", load_addr, unsafe {
            // *(load_addr as *const u8)
            // });

            // scheduler.spawn_raw(ptr, 0, &mut mapper, &mut frame_allocator);
        });

        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        // Allocate the Clock on the heap and store a 'static reference
        let clock_box = Box::new(Clock::new(hpet, rtc));
        let clock_static: &'static Clock = Box::leak(clock_box);
        unsafe { CLOCK = Some(clock_static) };

        info!("ThingOS initialized.");
        Self { framebuffer }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();
        info!("Interrupts enabled.");
        let scheduler = SCHEDULER.lock();

        scheduler.start_first();
        loop {
            info!("En attendant de nouvelles tâches...");
            hlt();
            //
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

#[unsafe(no_mangle)]
extern "C" fn hello_thread() {
    loop {
        serial_print!("1");
        // x86_64::instructions::hlt();
        // Simulate some work
        // for _ in 0..1_000_000u64 {
        //     //     // Busy wait
        // }
    }
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
extern "C" fn hello2_thread() {
    loop {
        serial_print!("2");
        // x86_64::instructions::hlt();
        // Simulate some work
        for _ in 0..1_000_000u64 {
            //     // Busy wait
        }
    }
}

pub const USER_BINARY_LOAD_BASE: u64 = 0xffff_8800_020_0000;

pub fn load_raw_binary(
    binary: &[u8],
    load_addr: u64,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
) -> *const u8 {
    use x86_64::VirtAddr;
    use x86_64::structures::paging::{Page, PageTableFlags as Flags};

    let aligned_load_addr = load_addr & !0xFFF;
    let offset = load_addr - aligned_load_addr;
    let total_len = offset + binary.len() as u64;
    let load_pages = (total_len + 0xFFF) / 0x1000;
    let base_page = Page::containing_address(VirtAddr::new(aligned_load_addr));

    for i in 0..load_pages {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical memory for binary");

        unsafe {
            mapper
                .map_to(
                    base_page + i,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                    frame_allocator,
                )
                .expect("map_to failed")
                .flush();
        }
    }

    unsafe {
        core::ptr::copy_nonoverlapping(binary.as_ptr(), load_addr as *mut u8, binary.len());
    }

    load_addr as *const u8
}

use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageTableFlags as Flags},
};

pub fn load_elf_executable(
    elf: &[u8],
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
) -> *const u8 {
    // --- Validate ELF header ---
    assert_eq!(&elf[0..4], b"\x7FELF", "Not a valid ELF file");

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
        let p_vaddr = u64::from_le_bytes(ph[0x10..0x18].try_into().unwrap());
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

        // for page in page_range {
        //     let frame = frame_allocator.allocate_frame().expect("Out of frames");
        //     unsafe {
        //         mapper
        //             .map_to(
        //                 page,
        //                 frame,
        //                 Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
        //                 frame_allocator,
        //             )
        //             .expect("map_to failed")
        //             .flush();
        //     }
        // }

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

    e_entry as *const u8
}
