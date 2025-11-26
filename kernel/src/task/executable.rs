use crate::arch::x86_64::memory::{kernel_base, kernel_end};
use crate::arch::x86_64::stack::{KERNEL_STACK_PAGES, KERNEL_STACK_VIRT_BASE};
use crate::bootloader::get_hhdm_offset;
use crate::mm::allocator::{HEAP_SIZE, HEAP_START};
use crate::mm::mirror_region::mirror_kernel_region;
use crate::task::context::{FullContext, TaskMode, prepare_context};
use crate::task::runtime;
use crate::task::scheduler::Task;
use core::ptr;
use goblin::elf::Elf;
use log::info;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
};

#[derive(Debug, Clone, Copy)]
pub struct LoadedElf {
    pub entry: VirtAddr,
    pub stack_top: VirtAddr,
}

pub fn create_user_page_table(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
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
    let l4_table = unsafe {
        let ptr: *mut PageTable = virt.as_mut_ptr();
        // Zero the page table in place to avoid a 4 KiB stack allocation from PageTable::new()
        ptr::write_bytes(ptr, 0, 1);
        let table = &mut *ptr;
        table.zero();
        table
    };
    // Reuse the existing HHDM PML4 entry so kernel code running with the user
    // address space still has the higher-half direct map available.
    let hhdm_index = hhdm_offset.p4_index();
    let active_cr3 = Cr3::read().0.start_address();
    let active_l4_virt = hhdm_offset + active_cr3.as_u64();
    let active_l4: &PageTable = unsafe { &*active_l4_virt.as_ptr() };
    l4_table[hhdm_index] = active_l4[hhdm_index].clone();
    let l4_table_ptr: *mut PageTable = l4_table;
    let mut offset_page_table = unsafe {
        x86_64::structures::paging::OffsetPageTable::new(&mut *l4_table_ptr, hhdm_offset)
    };
    // Copy kernel mappings into the new user page table so kernel code and data
    // remain accessible when the address space is switched.
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (kernel_base()..kernel_end()).into(),
    );
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (VirtAddr::new(HEAP_START)..VirtAddr::new(HEAP_START + HEAP_SIZE as u64)).into(),
    );
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (VirtAddr::new(KERNEL_STACK_VIRT_BASE)
            ..VirtAddr::new(KERNEL_STACK_VIRT_BASE + (KERNEL_STACK_PAGES as u64 * 4096)))
            .into(),
    );

    // Mirror task stacks (scheduler uses a different region)
    const TASK_STACK_REGION_BASE: u64 = 0xffff_8800_1000_0000;
    const MAX_TASKS_TO_MAP: u64 = 64;
    const TASK_STACK_SIZE: u64 = 16 * 4096;
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (VirtAddr::new(TASK_STACK_REGION_BASE)
            ..VirtAddr::new(TASK_STACK_REGION_BASE + MAX_TASKS_TO_MAP * TASK_STACK_SIZE))
            .into(),
    );

    // Ensure MMIO regions like the local APIC remain accessible.
    const APIC_BASE: u64 = 0xfee0_0000;
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (VirtAddr::new(APIC_BASE)..VirtAddr::new(APIC_BASE + 0x1000)).into(),
    );

    const HPET_BASE: u64 = 0xfed0_0000;
    mirror_kernel_region(
        &mut offset_page_table,
        frame_allocator,
        (VirtAddr::new(HPET_BASE)..VirtAddr::new(HPET_BASE + 0x1000)).into(),
    );

    (l4_table, offset_page_table)
}

pub fn load_elf<'a>(
    data: &[u8],
    _page_table: &mut PageTable,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<LoadedElf, &'static str> {
    let elf = Elf::parse(data).map_err(|_| "Failed to parse ELF")?;
    let load_base = VirtAddr::new(0x0000_4000_0000_0000);
    let user_stack_size = 16 * 4096;
    let user_stack_top = VirtAddr::new(0x0000_7000_0000_0000);
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

    for ph in &elf.program_headers {
        info!("Processing program header: {:?}", ph);
        if ph.p_type != goblin::elf::program_header::PT_LOAD {
            continue;
        }

        let file_offset = ph.p_offset as usize;
        let file_size = ph.p_filesz as usize;
        let mem_size = ph.p_memsz as usize;
        let vaddr = if elf.header.e_type == goblin::elf::header::ET_DYN {
            load_base + ph.p_vaddr
        } else {
            VirtAddr::new(ph.p_vaddr)
        };
        let end_vaddr = vaddr + mem_size as u64;

        let start_page = Page::containing_address(vaddr);
        let end_page = Page::containing_address(end_vaddr - 1u64);

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if ph.is_write() {
            flags |= PageTableFlags::WRITABLE;
        }
        if !ph.is_executable() {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        for page in Page::range_inclusive(start_page, end_page) {
            if mapper.translate_page(page).is_ok() {
                continue;
            }
            let p4_index = page.p4_index();
            unsafe {
                let entry = &_page_table[p4_index];
                log::debug!(
                    "P4[{:#x}] flags={:?} addr={:#x}",
                    u64::from(p4_index),
                    entry.flags(),
                    entry.addr().as_u64()
                );
                if !entry.is_unused() {
                    let hhdm = get_hhdm_offset().as_u64();
                    let p3_ptr = (hhdm + entry.addr().as_u64()) as *const PageTable;
                    let p3 = &*p3_ptr;
                    let p3_entry = &p3[page.p3_index()];
                    log::debug!(
                        "  P3[{:#x}] flags={:?} addr={:#x}",
                        u64::from(page.p3_index()),
                        p3_entry.flags(),
                        p3_entry.addr().as_u64()
                    );
                    if !p3_entry.is_unused()
                        && !p3_entry.flags().contains(PageTableFlags::HUGE_PAGE)
                    {
                        let p2_ptr = (hhdm + p3_entry.addr().as_u64()) as *const PageTable;
                        let p2 = &*p2_ptr;
                        let p2_entry = &p2[page.p2_index()];
                        log::debug!(
                            "    P2[{:#x}] flags={:?} addr={:#x}",
                            u64::from(page.p2_index()),
                            p2_entry.flags(),
                            p2_entry.addr().as_u64()
                        );
                    }
                }
            }
            let frame = frame_allocator
                .allocate_frame()
                .ok_or("Failed to allocate frame")?;

            info!(
                "Mapping page {:#x} to frame {:#x} with flags {:?}",
                page.start_address().as_u64(),
                frame.start_address().as_u64(),
                flags
            );

            unsafe {
                mapper
                    .map_to(page, frame, flags, frame_allocator)
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

        let seg_start = vaddr.as_u64();
        let file_end = seg_start + file_size as u64;
        let mem_end = seg_start + mem_size as u64;
        let hhdm = get_hhdm_offset().as_u64();

        for page in Page::range_inclusive(start_page, end_page) {
            let page_start = page.start_address().as_u64();
            let page_end = page_start + 0x1000;
            let frame = mapper
                .translate_page(page)
                .map_err(|_| "Failed to translate page for copy")?;
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
                    unsafe {
                        core::ptr::write_bytes(dst_ptr, 0, (zero_end - zero_start) as usize);
                    }
                }
            }
        }

        info!(
            "Copied {} bytes from file offset {} to virtual address {:?}",
            file_size, file_offset, vaddr
        );
    }

    let entry = if elf.header.e_type == goblin::elf::header::ET_DYN {
        load_base + elf.entry
    } else {
        VirtAddr::new(elf.entry)
    };

    Ok(LoadedElf {
        entry,
        stack_top: user_stack_top,
    })
}

pub unsafe fn jump_to_context(ctx: &FullContext, new_table: PhysFrame) -> ! {
    info!(
        "Jumping to task with new page table {:#x} (rip={:#x}, rsp={:#x}, cs={:#x}, ss={:#x})",
        new_table.start_address().as_u64(),
        ctx.frame.rip,
        ctx.frame.rsp,
        ctx.frame.cs,
        ctx.frame.ss
    );
    unsafe {
        Cr3::write(new_table, Cr3::read().1);
    }
    unsafe extern "C" {
        fn restore_context(saved: *const u8) -> !;
    }
    unsafe { restore_context(ctx as *const _ as *const u8) };
}

pub unsafe fn jump_to_user(entry: VirtAddr, stack_top: VirtAddr, new_table: PhysFrame) -> ! {
    let entry_fn: extern "C" fn() = unsafe { core::mem::transmute(entry.as_u64()) };
    let ctx = prepare_context(entry_fn, stack_top.as_u64(), TaskMode::User);
    unsafe { jump_to_context(&ctx, new_table) };
}
