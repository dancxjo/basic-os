use crate::memory::{kernel_base, kernel_end};
use crate::mirror_region::mirror_kernel_region;
use crate::stack::{KERNEL_STACK_PAGES, KERNEL_STACK_VIRT_BASE};
use goblin::elf::Elf;
use log::info;
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
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
        ptr.write(PageTable::new());
        &mut *ptr
    };
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
        (VirtAddr::new(KERNEL_STACK_VIRT_BASE)
            ..VirtAddr::new(KERNEL_STACK_VIRT_BASE + (KERNEL_STACK_PAGES as u64 * 4096)))
            .into(),
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
        let vaddr = load_base + ph.p_vaddr;
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

        let src = &data[file_offset..file_offset + file_size];

        if let Some(frame) = mapper.translate_page(Page::containing_address(vaddr)).ok() {
            let phys = frame.start_address();
            let dst_ptr = (phys.as_u64() + 0xffff_8000_0000_0000) as *mut u8;
            let offset = (vaddr.as_u64() & 0xfff) as usize;
            info!(
                "Copying segment: dst={:#x}, offset_in_page={:#x}, file_size={}, mem_size={}",
                dst_ptr as u64, offset, file_size, mem_size
            );
            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), dst_ptr.add(offset), file_size);
                core::ptr::write_bytes(dst_ptr.add(offset + file_size), 0, mem_size - file_size);
            }
        } else {
            return Err("Failed to translate page for copy");
        }

        info!(
            "Copied {} bytes from file offset {} to virtual address {:?}",
            file_size, file_offset, vaddr
        );
    }

    Ok(LoadedElf {
        entry: load_base + elf.entry,
        stack_top: user_stack_top,
    })
}

pub unsafe fn jump_to_user(entry: VirtAddr, stack_top: VirtAddr, new_table: PhysFrame) -> ! {
    info!(
        "Jumping to user mode with entry: {:#x}, stack_top: {:#x}",
        entry.as_u64(),
        stack_top.as_u64()
    );

    // Switch to the new address space
    unsafe { Cr3::write(new_table, Cr3::read().1) };
    info!(
        "Switched to new address space: {:#x}",
        new_table.start_address().as_u64()
    );
    use crate::gdt::SELECTORS;
    #[allow(static_mut_refs)]
    let selectors = unsafe { SELECTORS.as_ref().unwrap() };
    let user_data_sel = selectors.user_data;
    let user_code_sel = selectors.user_code;

    info!("About to enter user mode:");
    info!("  entry     = {:#x}", entry.as_u64());
    info!("  stack_top = {:#x}", stack_top.as_u64());
    info!("  user_code_sel = {:#x}", user_code_sel.0);
    info!("  user_data_sel = {:#x}", user_data_sel.0);

    // Dump the first few bytes at the entry point for debugging
    let code_ptr = entry.as_u64() as *const u8;
    let code_slice = core::slice::from_raw_parts(code_ptr, 16);
    info!("Entry code bytes: {:02x?}", code_slice);

    // Set up stack frame for iretq
    unsafe {
        core::arch::asm!(
            "cli", // Disable interrupts

            // Push fake stack frame expected by iretq
            "push {user_data}",   // SS
            "push {stack}",       // RSP
            "pushf",              // RFLAGS
            "push {user_code}",   // CS
            "push {entry}",       // RIP
            "iretq",

            user_data = in(reg) u64::from(user_data_sel.0),
            stack     = in(reg) stack_top.as_u64(),
            user_code = in(reg) u64::from(user_code_sel.0),
            entry     = in(reg) entry.as_u64(),

            options(noreturn)
        );
    }
}
