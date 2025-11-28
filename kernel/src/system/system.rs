use log::info;
use spin::mutex::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, OffsetPageTable};

use crate::arch::x86_64::gdt::init_gdt;
use crate::arch::x86_64::idt::init_idt;
use crate::arch::x86_64::interrupts::init_interrupts;
use crate::arch::x86_64::ps2;
use crate::arch::x86_64::stack::init_kernel_stack;
use crate::bootloader::{get_hhdm_offset, get_module, list_modules};
use crate::bootstrap_step;
use crate::clock::{Clock, HPET, RTC};
use crate::drivers::framebuffer::{Framebuffer, init_console, register_framebuffer_device};
use crate::drivers::{keyboard, serial};
use crate::mm::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::task::executable::{create_user_page_table, jump_to_user, load_elf};
use crate::task::runtime;
use crate::telemetry::canon;
use crate::telemetry::graph::{self, BundleId, BundleType};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
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
    clock: &'static SpinMutex<Clock>,
    // scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
}

#[derive(Clone)]
struct UserModule {
    name: &'static str,
    bundle: BundleId,
    bundle_type: BundleType,
}

static USER_MODULES: spin::Mutex<Option<Vec<UserModule>>> = spin::Mutex::new(None);
static NEXT_USER_MODULE: AtomicUsize = AtomicUsize::new(0);

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
            // Test allocator immediately after init
            let mut v = alloc::vec::Vec::new();
            v.push(42);
            log::info!("Allocator test in boot: v[0] = {}", v[0]);
        });

        bootstrap_step!("telemetry", {
            crate::telemetry::init();
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
            if let Err(err) = crate::drivers::mouse::init() {
                info!("Mouse driver init failed: {}", err);
            }
        });

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let _clock = Box::leak(Box::new(SpinMutex::new(Clock::new(hpet, rtc))));
        crate::clock::set_global_clock(_clock);
        info!("ThingOS initialized.");

        bootstrap_step!("executable", {
            init_user_modules();
            let count = user_module_count();
            if count == 0 {
                info!("No user modules to launch.");
            }
            for _ in 0..count {
                runtime::spawn_kernel(start_user_task);
            }
        });

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
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}

fn init_user_modules() {
    let mut modules: Vec<&'static str> = Vec::new();
    for (name, data) in crate::bootloader::list_modules().into_iter() {
        if data.starts_with(b"\x7FELF") {
            info!("Queueing user module '{}'", name);
            modules.push(name);
        } else {
            info!("Skipping non-ELF module '{}'", name);
        }
    }
    let mut entries = Vec::new();
    for name in modules.into_iter() {
        // Infer bundle type from module name
        let bundle_type = BundleType::from_name(name);

        // Create the bundle with proper type information using the lifecycle API
        let bundle = graph::create_bundle(name, bundle_type, None);

        info!(
            "Created bundle for '{}' with type {:?}, id={}",
            name, bundle_type, bundle
        );

        entries.push(UserModule {
            name,
            bundle,
            bundle_type,
        });
    }
    *USER_MODULES.lock() = Some(entries);
    NEXT_USER_MODULE.store(0, Ordering::Release);
}

fn next_user_module() -> Option<UserModule> {
    let guard = USER_MODULES.lock();
    let list = guard.as_ref()?;
    let idx = NEXT_USER_MODULE.fetch_add(1, Ordering::AcqRel);
    list.get(idx).cloned()
}

fn user_module_count() -> usize {
    USER_MODULES.lock().as_ref().map(|v| v.len()).unwrap_or(0)
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

#[unsafe(no_mangle)]
pub extern "C" fn start_user_task() {
    let module = next_user_module().unwrap_or_else(|| {
        info!("No remaining user modules to start; halting task.");
        loop {}
    });

    // Assign the bundle to the current task before first run
    runtime::assign_current_bundle(module.bundle);

    // Grant initial capabilities based on bundle type
    grant_initial_capabilities(&module);

    let module_bytes = get_module(module.name).unwrap_or_else(|| {
        panic!("Module '{}' not found", module.name);
    });
    let frame_allocator = BootFrameAllocator::global();
    let (new_l4, mut new_mapper) = create_user_page_table(frame_allocator, get_hhdm_offset());
    let loaded = load_elf(module_bytes, new_l4, &mut new_mapper, frame_allocator)
        .expect("Failed to load ELF");
    info!(
        "User entry prepared for {}: rip={:#x} stack_top={:#x}",
        module.name,
        loaded.entry.as_u64(),
        loaded.stack_top.as_u64()
    );
    let new_table_frame = PhysFrame::containing_address(PhysAddr::new(
        new_l4 as *const _ as u64 - get_hhdm_offset().as_u64(),
    ));
    unsafe {
        jump_to_user(loaded.entry, loaded.stack_top, new_table_frame);
    }
}

/// Grant initial capabilities to a bundle based on its type.
/// - Drivers get access to device nodes they're responsible for
/// - Compositors get framebuffer access
/// - Apps get minimal initial capabilities
fn grant_initial_capabilities(module: &UserModule) {
    use uuid::Uuid;

    match module.bundle_type {
        BundleType::Driver => {
            // Grant driver capabilities based on name
            if module.name.contains("keyboard") {
                // Keyboard driver gets input device capability
                info!("Granting keyboard driver capabilities to {}", module.name);
            } else if module.name.contains("mouse") {
                // Mouse driver gets input device capability
                info!("Granting mouse driver capabilities to {}", module.name);
            } else if module.name.contains("framebuffer") {
                // Framebuffer driver gets display device capability
                let framebuffer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"framebuffer0");
                if graph::grant_initial_capability(module.bundle, framebuffer_id, canon::CAN_WRITE)
                {
                    info!(
                        "Granted CAN_WRITE on framebuffer to bundle {}",
                        module.bundle
                    );
                }
                if graph::grant_initial_capability(module.bundle, framebuffer_id, canon::CAN_READ) {
                    info!(
                        "Granted CAN_READ on framebuffer to bundle {}",
                        module.bundle
                    );
                }
            }
        }
        BundleType::Compositor => {
            // Compositor gets framebuffer access for display composition
            let framebuffer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"framebuffer0");
            if graph::grant_initial_capability(module.bundle, framebuffer_id, canon::CAN_WRITE) {
                info!(
                    "Granted CAN_WRITE on framebuffer to compositor {}",
                    module.bundle
                );
            }
            if graph::grant_initial_capability(module.bundle, framebuffer_id, canon::CAN_READ) {
                info!(
                    "Granted CAN_READ on framebuffer to compositor {}",
                    module.bundle
                );
            }
        }
        BundleType::App => {
            // Apps start with minimal capabilities
            // They can request additional capabilities through syscalls
            info!("App {} starting with minimal capabilities", module.name);
        }
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
