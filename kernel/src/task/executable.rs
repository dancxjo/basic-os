use crate::arch::x86_64::memory::{kernel_base, kernel_end};
use crate::arch::x86_64::stack::{KERNEL_STACK_PAGES, KERNEL_STACK_VIRT_BASE};
use crate::bootloader::get_hhdm_offset;
use crate::mm::allocator::{HEAP_SIZE, HEAP_START};
use crate::mm::mirror_region::mirror_kernel_region;
use crate::task::context::{FullContext, IretFrame, TaskMode, prepare_context};
use crate::task::scheduler::Task;
use core::ptr;
use goblin::elf::Elf;
use log::info;
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate,
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
    let l4_table = unsafe {
        let ptr: *mut PageTable = virt.as_mut_ptr();
        // Zero the page table in place to avoid a 4 KiB stack allocation from PageTable::new()
        ptr::write_bytes(ptr, 0, 1);
        let table = &mut *ptr;
        table.zero();
        table
    };
    // Copy all higher-half kernel mappings (indices 256-511) to ensure the kernel
    // has full access to its address space (HHDM, kernel code, etc.) while running
    // in the user's context.
    let active_cr3 = Cr3::read().0.start_address();
    let active_l4_virt = hhdm_offset + active_cr3.as_u64();
    let active_l4: &PageTable = unsafe { &*active_l4_virt.as_ptr() };

    for i in 256..512 {
        l4_table[i] = active_l4[i].clone();
    }

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
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (kernel_base()..kernel_end()).into(),
    );
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (VirtAddr::new(HEAP_START)..VirtAddr::new(HEAP_START + HEAP_SIZE as u64)).into(),
    );
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (VirtAddr::new(KERNEL_STACK_VIRT_BASE)
            ..VirtAddr::new(KERNEL_STACK_VIRT_BASE + (KERNEL_STACK_PAGES as u64 * 4096)))
            .into(),
    );

    // Mirror task stacks (scheduler uses a different region)
    const TASK_STACK_REGION_BASE: u64 = 0xffff_8800_1000_0000;
    const MAX_TASKS_TO_MAP: u64 = 64;
    let task_stack_size = Task::stack_size(); // Keep mirrored size in sync with scheduler stacks.
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (VirtAddr::new(TASK_STACK_REGION_BASE)
            ..VirtAddr::new(TASK_STACK_REGION_BASE + MAX_TASKS_TO_MAP * task_stack_size))
            .into(),
    );

    // Ensure MMIO regions like the local APIC remain accessible.
    const APIC_BASE: u64 = 0xfee0_0000;
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (VirtAddr::new(APIC_BASE)..VirtAddr::new(APIC_BASE + 0x1000)).into(),
    );

    const HPET_BASE: u64 = 0xfed0_0000;
    mirror_kernel_region(
        &mut offset_page_table,
        active_mapper,
        frame_allocator,
        (VirtAddr::new(HPET_BASE)..VirtAddr::new(HPET_BASE + 0x1000)).into(),
    );

    // Verify kernel mapping
    let kernel_func_addr = VirtAddr::new(jump_to_user as usize as u64);
    if offset_page_table.translate_addr(kernel_func_addr).is_none() {
        panic!("Kernel code not mapped in user page table!");
    }

    // Verify HHDM mapping (check a known HHDM address, e.g. active_l4_virt)
    if offset_page_table.translate_addr(active_l4_virt).is_none() {
        panic!("HHDM not mapped in user page table!");
    }

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
                info!(
                    "Page {:#x} already mapped, skipping",
                    page.start_address().as_u64()
                );
                continue;
            }

            let frame = frame_allocator
                .allocate_frame()
                .ok_or("Failed to allocate frame")?;
            // Leave this commented out as it slows copying significantly
            // info!(
            //     "Mapping page {:#x} to frame {:#x} with flags {:?}",
            //     page.start_address().as_u64(),
            //     frame.start_address().as_u64(),
            //     flags
            // );

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
        fn restore_context(saved: *const IretFrame) -> !;
    }
    unsafe { restore_context(&ctx.frame) };
}

pub unsafe fn jump_to_user(entry: VirtAddr, stack_top: VirtAddr, new_table: PhysFrame) -> ! {
    let entry_fn: extern "C" fn() = unsafe { core::mem::transmute(entry.as_u64()) };
    let ctx = prepare_context(entry_fn, stack_top.as_u64(), TaskMode::User);
    unsafe { jump_to_context(&ctx, new_table) };
}
