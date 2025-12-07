use crate::arch::x86_64::memory::{kernel_base, kernel_end};
extern crate alloc;
use crate::arch::x86_64::gdt::set_kernel_stack;
use crate::arch::x86_64::stack::{KERNEL_STACK_PAGES, KERNEL_STACK_TOP, KERNEL_STACK_VIRT_BASE};
use crate::bootloader::get_hhdm_offset;
use crate::mm::allocator::{HEAP_SIZE, HEAP_START};
use crate::mm::layout::{
    KERNEL_STACK_REGION_BASE, MAX_TASKS_TO_MAP, USER_CODE_BASE, USER_STACK_SIZE, USER_STACK_TOP,
};
use crate::mm::mirror_region::mirror_kernel_region;
use crate::task::context::{FullContext, IretFrame, TaskMode, prepare_context};
use crate::task::elf::{ET_DYN, Elf, PF_W, PF_X, PT_LOAD, ProgramHeader};
use crate::task::scheduler::Task;
use alloc::boxed::Box;
use core::ptr;
use log::info;
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate,
        mapper::TranslateResult,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct LoadedElf {
    pub entry: VirtAddr,
    pub stack_top: VirtAddr,
}

pub fn create_user_page_table(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    active_mapper: &mut (impl Mapper<Size4KiB> + Translate),
    hhdm_offset: VirtAddr,
) -> (
    &'static mut PageTable,
    x86_64::structures::paging::OffsetPageTable<'static>,
) {
    let l4_frame = frame_allocator
        .allocate_frame()
        .expect("No frame for user L4 page table");
    let phys = l4_frame.start_address();
    let virt = hhdm_offset + phys.as_u64();
    info!(
        "Allocated new user L4 table at {:?} (phys {:?}) HHDM: {:?}",
        virt, phys, hhdm_offset
    );
    let l4_table = unsafe {
        let ptr: *mut PageTable = virt.as_mut_ptr();
        // Zero the page table in place to avoid a 4 KiB stack allocation from PageTable::new()
        ptr::write_bytes(ptr, 0, 1);
        let table = &mut *ptr;
        table.zero();
        table
    };
    info!("DEBUG: l4_table initialized");
    // Copy all higher-half kernel mappings (indices 256-511) to ensure the kernel
    // has full access to its address space (HHDM, kernel code, etc.) while running
    // in the user's context.
    let active_cr3 = Cr3::read().0.start_address();
    let active_l4_virt = hhdm_offset + active_cr3.as_u64();
    let active_l4: &PageTable = unsafe { &*active_l4_virt.as_ptr() };

    for i in 256..512 {
        l4_table[i] = active_l4[i].clone();
    }
    info!("DEBUG: Kernel mappings copied");
    info!("DEBUG: active_l4[511] = {:?}", active_l4[511]);
    info!("DEBUG: l4_table[511] = {:?}", l4_table[511]);

    // Verify HHDM is present (it should be in the copied range)
    let hhdm_index = hhdm_offset.p4_index();
    if !l4_table[hhdm_index]
        .flags()
        .contains(PageTableFlags::PRESENT)
    {
        panic!("HHDM PML4 entry not present in active page table!");
    }

    let l4_table_ptr: *mut PageTable = l4_table;
    let mut offset_page_table = unsafe {
        x86_64::structures::paging::OffsetPageTable::new(&mut *l4_table_ptr, hhdm_offset)
    };
    // Copy kernel mappings into the new user page table so kernel code and data
    // remain accessible when the address space is switched.
    // info!("Mirroring kernel region...");
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (kernel_base()..kernel_end()).into(),
    // );
    // info!("Mirroring heap region...");
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (VirtAddr::new(HEAP_START)..VirtAddr::new(HEAP_START + HEAP_SIZE as u64)).into(),
    // );
    // info!("Mirroring kernel stack region...");
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (VirtAddr::new(KERNEL_STACK_VIRT_BASE)
    //         ..VirtAddr::new(KERNEL_STACK_VIRT_BASE + (KERNEL_STACK_PAGES as u64 * 4096)))
    //         .into(),
    // );

    // Mirror task stacks (scheduler uses a different region)
    // const TASK_STACK_REGION_BASE: u64 = 0xffff_8800_1000_0000;
    // const MAX_TASKS_TO_MAP: u64 = 64;
    let task_stack_size = Task::stack_size(); // Keep mirrored size in sync with scheduler stacks.
    // info!("Mirroring task stack region...");
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (VirtAddr::new(KERNEL_STACK_REGION_BASE)
    //         ..VirtAddr::new(KERNEL_STACK_REGION_BASE + MAX_TASKS_TO_MAP * task_stack_size))
    //         .into(),
    // );

    // Ensure MMIO regions like the local APIC remain accessible.
    const APIC_BASE: u64 = 0xfee0_0000;
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (VirtAddr::new(APIC_BASE)..VirtAddr::new(APIC_BASE + 0x1000)).into(),
    // );

    const HPET_BASE: u64 = 0xfed0_0000;
    // mirror_kernel_region(
    //     &mut offset_page_table,
    //     active_mapper,
    //     frame_allocator,
    //     (VirtAddr::new(HPET_BASE)..VirtAddr::new(HPET_BASE + 0x1000)).into(),
    // );

    // Verify kernel mapping
    let kernel_func_addr = VirtAddr::new(jump_to_user as usize as u64);
    if offset_page_table.translate_addr(kernel_func_addr).is_none() {
        panic!("Kernel code not mapped in user page table!");
    }

    // Verify HHDM mapping (check a known HHDM address, e.g. active_l4_virt)
    if offset_page_table.translate_addr(active_l4_virt).is_none() {
        panic!("HHDM not mapped in user page table!");
    }
    info!("DEBUG: create_user_page_table finished");

    (l4_table, offset_page_table)
}

fn elf_flags_to_pt_flags(ph: &ProgramHeader) -> PageTableFlags {
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    if (ph.p_flags & PF_W) != 0 {
        flags |= PageTableFlags::WRITABLE;
    }
    if (ph.p_flags & PF_X) == 0 {
        flags |= PageTableFlags::NO_EXECUTE;
    }
    flags
}

use x86_64::instructions::interrupts;

pub fn load_elf<'a>(
    data: &[u8],
    _page_table: &mut PageTable,
    mapper: &mut (impl Mapper<Size4KiB> + Translate),
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<LoadedElf, &'static str> {
    interrupts::without_interrupts(|| load_elf_inner(data, _page_table, mapper, frame_allocator))
}

fn load_elf_inner<'a>(
    data: &[u8],
    _page_table: &mut PageTable,
    mapper: &mut (impl Mapper<Size4KiB> + Translate),
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<LoadedElf, &'static str> {
    // info!("DEBUG: load_elf start. data len: {}", data.len());
    let elf = Elf::parse(data).map_err(|_| "Failed to parse ELF")?;
    // info!("DEBUG: ELF parsed successfully");
    let load_base = VirtAddr::new(USER_CODE_BASE);
    let user_stack_size = USER_STACK_SIZE;
    let user_stack_top = VirtAddr::new(USER_STACK_TOP);
    let user_stack_start = user_stack_top - user_stack_size as u64;

    // Map user stack
    let start_page = Page::containing_address(user_stack_start);
    let end_page = Page::containing_address(user_stack_top - 1u64);
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or("Failed to allocate user stack frame")?;
        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .map_err(|_| "Failed to map user stack page")?
                .flush();
        }
    }

    for ph in elf.program_headers {
        info!("Processing program header: {:?}", ph);
        if ph.p_type != PT_LOAD {
            continue;
        }

        let file_offset = ph.p_offset as usize;
        let file_size = ph.p_filesz as usize;
        let mem_size = ph.p_memsz as usize;
        let vaddr = if elf.header.e_type == ET_DYN {
            load_base + ph.p_vaddr
        } else {
            VirtAddr::new(ph.p_vaddr)
        };
        let end_vaddr = vaddr + mem_size as u64;

        let start_page = Page::containing_address(vaddr);
        let mut end_page = Page::containing_address(end_vaddr - 1u64);

        // if mem_size > 1000000 {
        //     info!("DEBUG: Skipping large segment for testing");
        //     continue;
        // }

        let desired_flags = elf_flags_to_pt_flags(ph);

        info!(
            "Mapping large segment: vaddr={:?}, mem_size={}, pages={}",
            vaddr,
            mem_size,
            (end_page - start_page) + 1
        );

        for page in Page::range_inclusive(start_page, end_page) {
            // Check if we need to yield or re-enable interrupts briefly to avoid watchdog timeouts
            // But since we are in without_interrupts, we can't.
            // However, the crash is a Page Fault caused by WRITE at 0xffffffff800662d7
            // which is inside without_interrupts closure.

            match mapper.translate(page.start_address()) {
                TranslateResult::Mapped {
                    flags: existing_flags,
                    ..
                } => {
                    let mut new_flags = existing_flags;
                    let mut needs_update = false;

                    if desired_flags.contains(PageTableFlags::WRITABLE)
                        && !existing_flags.contains(PageTableFlags::WRITABLE)
                    {
                        new_flags |= PageTableFlags::WRITABLE;
                        needs_update = true;
                    }

                    if existing_flags.contains(PageTableFlags::NO_EXECUTE)
                        && !desired_flags.contains(PageTableFlags::NO_EXECUTE)
                    {
                        new_flags.remove(PageTableFlags::NO_EXECUTE);
                        needs_update = true;
                    }

                    if needs_update {
                        info!(
                            "DEBUG: about to upgrade flags for page {:#x} (segment flags={:#x})",
                            page.start_address().as_u64(),
                            ph.p_flags
                        );
                        info!(
                            "Upgrading page {:#x} flags from {:#x} to {:#x}",
                            page.start_address().as_u64(),
                            existing_flags.bits(),
                            new_flags.bits()
                        );
                        unsafe {
                            mapper
                                .update_flags(page, new_flags)
                                .map_err(|_| "Failed to update flags")?
                                .flush();
                        }
                    } else {
                        info!(
                            "Page {:#x} already mapped with sufficient flags, skipping remap",
                            page.start_address().as_u64()
                        );
                    }
                }
                TranslateResult::NotMapped => {
                    let frame = frame_allocator
                        .allocate_frame()
                        .ok_or("Failed to allocate frame")?;

                    if frame.start_address().as_u64() < 0x4102000 {
                        log::error!(
                            "Allocated frame {:#x} inside stack/L4 range!",
                            frame.start_address().as_u64()
                        );
                        panic!("Frame collision!");
                    }

                    unsafe {
                        mapper
                            .map_to(page, frame, desired_flags, frame_allocator)
                            .map_err(|e| {
                                info!(
                                    "map_to failed for page {:#x} -> frame {:#x}: {:?}",
                                    page.start_address().as_u64(),
                                    frame.start_address().as_u64(),
                                    e
                                );
                                "Failed to map page"
                            })?
                            .flush();
                    }
                }
                _ => return Err("Invalid translation result"),
            }
        }

        info!("DEBUG: Mapping finished. Starting copy/zero loop.");

        let seg_start = vaddr.as_u64();
        let file_end = seg_start + file_size as u64;
        let mem_end = seg_start + mem_size as u64;
        let hhdm = get_hhdm_offset().as_u64();

        for page in Page::range_inclusive(start_page, end_page) {
            let page_start = page.start_address().as_u64();
            let page_end = page_start + 0x1000;

            let frame = match mapper.translate(page.start_address()) {
                TranslateResult::Mapped { frame, .. } => frame,
                _ => return Err("Failed to translate page for copy"),
            };

            let dst_base = hhdm + frame.start_address().as_u64();

            // Copy the portion of the file that falls into this page.
            if page_start < file_end {
                let copy_start = core::cmp::max(page_start, seg_start);
                let copy_end = core::cmp::min(page_end, file_end);
                if copy_start < copy_end {
                    let src_off = (copy_start - seg_start) as usize;
                    let len = (copy_end - copy_start) as usize;
                    let dst_ptr = (dst_base + (copy_start - page_start)) as *mut u8;
                    let src_slice =
                        &data[file_offset as usize + src_off..file_offset as usize + src_off + len];
                    unsafe {
                        core::ptr::copy_nonoverlapping(src_slice.as_ptr(), dst_ptr, len);
                    }
                }
            }

            // Zero any remaining BSS in this page.
            if page_start < mem_end {
                let zero_start = core::cmp::max(page_start, seg_start + file_size as u64);
                let zero_end = core::cmp::min(page_end, mem_end);
                if zero_start < zero_end {
                    let dst_ptr = (dst_base + (zero_start - page_start)) as *mut u8;
                    /*
                    unsafe {
                        core::ptr::write_bytes(dst_ptr, 0, (zero_end - zero_start) as usize);
                    }
                    */
                }
            }
        }

        info!(
            "Copied {} bytes from file offset {} to virtual address {:?}",
            file_size, file_offset, vaddr
        );
    }

    let entry = if elf.header.e_type == ET_DYN {
        load_base + elf.header.e_entry
    } else {
        VirtAddr::new(elf.header.e_entry)
    };

    info!("DEBUG: load_elf_inner returning Ok");
    Ok(LoadedElf {
        entry,
        stack_top: user_stack_top,
    })
}

pub unsafe fn jump_to_context(ctx: &FullContext, new_table: PhysFrame) -> ! {
    crate::serial_println!("DEBUG: jump_to_context start");
    info!(
        "Jumping to task with new page table {:#x} (rip={:#x}, rsp={:#x}, cs={:#x}, ss={:#x})",
        new_table.start_address().as_u64(),
        ctx.frame.rip,
        ctx.frame.rsp,
        ctx.frame.cs,
        ctx.frame.ss
    );

    // Validate RIP
    let rip = ctx.frame.rip;
    let is_canonical = |v: u64| {
        let sign = v >> 47;
        sign == 0 || sign == 0x1ffff
    };
    assert!(is_canonical(rip), "Non-canonical RIP: {:#x}", rip);

    if ctx.frame.cs == 0x8 {
        // Kernel mode
        assert!(
            rip >= 0xffffffff80000000 && rip < 0xffffffff90000000,
            "RIP out of kernel text region: {:#x}",
            rip
        );
    } else {
        // User mode (approximate check)
        assert!(
            rip < 0x0000800000000000,
            "RIP out of user text region: {:#x}",
            rip
        );
    }

    unsafe {
        Cr3::write(new_table, Cr3::read().1);
    }
    unsafe extern "C" {
        fn restore_context(saved: *const FullContext) -> !;
    }
    unsafe { restore_context(ctx) };
}

pub unsafe fn jump_to_user(entry: VirtAddr, stack_top: VirtAddr, new_table: PhysFrame) -> ! {
    crate::serial_println!("DEBUG: jump_to_user start");
    // Do NOT reset the kernel stack to the boot stack.
    // We want to keep using the current task's kernel stack (which is already set).
    /*
    unsafe {
        if KERNEL_STACK_TOP.as_u64() != 0 {
            set_kernel_stack(KERNEL_STACK_TOP.as_u64());
        }
    }
    */
    let entry_fn: extern "C" fn() = unsafe { core::mem::transmute(entry.as_u64()) };
    let ctx = Box::new(prepare_context(
        entry_fn,
        stack_top.as_u64(),
        TaskMode::User,
    ));
    crate::serial_println!("DEBUG: calling jump_to_context");
    unsafe { jump_to_context(&*ctx, new_table) };
}

pub fn test_page_flag_upgrade(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    active_mapper: &mut (impl Mapper<Size4KiB> + Translate),
    hhdm_offset: VirtAddr,
) {
    info!("[TEST] page_flag_upgrade start");

    // Create a dummy user page table
    let (_l4_table, mut mapper) =
        create_user_page_table(frame_allocator, active_mapper, hhdm_offset);

    let test_virt = VirtAddr::new(0x4000_1000);
    let frame = frame_allocator.allocate_frame().expect("test frame alloc");

    info!(
        "[TEST] Mapping page {:#x} to frame {:#x} (READ_ONLY)",
        test_virt.as_u64(),
        frame.start_address().as_u64()
    );

    unsafe {
        mapper
            .map_to(
                Page::<Size4KiB>::containing_address(test_virt),
                frame,
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                frame_allocator,
            )
            .expect("map_to failed")
            .flush();
    }

    info!("[TEST] Upgrading flags to WRITABLE");
    let new_flags =
        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE;

    unsafe {
        mapper
            .update_flags(Page::<Size4KiB>::containing_address(test_virt), new_flags)
            .expect("update_flags failed")
            .flush();
    }

    info!("[TEST] page_flag_upgrade completed without fault");
}
