//! idt.rs — Interrupt Descriptor Table setup for ThingOS

#![allow(static_mut_refs)]

use crate::serial_println;
use x86_64::VirtAddr;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::structures::paging::{FrameAllocator, Page, PageTableFlags, PhysFrame, Size4KiB};

use crate::memory::{BootFrameAllocator, map_page_to};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_START: u64 = 0x4444_7000_0000;
const DOUBLE_FAULT_STACK_SIZE: usize = 5 * 4096; // 20 KiB

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        IDT.load();
    }
    serial_println!("IDT initialized and loaded.");
}

pub fn init_double_fault_stack(
    tss: &mut TaskStateSegment,
    mapper: &mut x86_64::structures::paging::OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
) {
    let start = VirtAddr::new(DOUBLE_FAULT_STACK_START);
    let end = start + DOUBLE_FAULT_STACK_SIZE;

    for page in Page::range_inclusive(
        Page::containing_address(start),
        Page::containing_address(end - 1u64),
    ) {
        let frame = frame_allocator.allocate_frame().expect("no frame");
        map_page_to(
            mapper,
            page,
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        );
    }

    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] =
        VirtAddr::new(DOUBLE_FAULT_STACK_START + DOUBLE_FAULT_STACK_SIZE as u64);

    serial_println!("Double fault IST stack mapped and configured.");
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    serial_println!(
        "EXCEPTION: PAGE FAULT\n{:#?}\nError code: {:?}",
        stack_frame,
        error_code
    );
    serial_println!(
        "Faulting address: {:?}",
        x86_64::registers::control::Cr2::read()
    );

    loop {}
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    serial_println!(
        "EXCEPTION: DOUBLE FAULT\n{:#?}\nError code: {:#x}",
        stack_frame,
        error_code
    );
    loop {}
}
