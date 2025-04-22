//! idt.rs — Interrupt Descriptor Table setup for ThingOS

#![allow(static_mut_refs)]

use log::{error, info};
use x86_64::VirtAddr;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use x86_64::structures::paging::{FrameAllocator, Page, PageTableFlags};
use x86_64::structures::tss::TaskStateSegment;

use crate::memory::{BootFrameAllocator, map_page_to};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_START: u64 = 0x4444_7000_0000;
const DOUBLE_FAULT_STACK_SIZE: usize = 5 * 4096; // 20 KiB

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,
    Keyboard = 33,
    Mouse = 44, // IRQ12
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    let addr = Cr2::read();

    error!(
        "\u{1F4A8} PAGE FAULT\nFaulting address: {:#018x}\nError code: {:?}\nInstruction pointer: {:#018x}\nStack pointer: {:#018x}\nCode segment:        {:#x}\nStack segment:       {:#x}\nCPU flags:           {:#x}",
        addr,
        error_code,
        stack_frame.instruction_pointer.as_u64(),
        stack_frame.stack_pointer.as_u64(),
        stack_frame.code_segment,
        stack_frame.stack_segment,
        stack_frame.cpu_flags,
    );

    loop {}
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    error!(
        "EXCEPTION: DOUBLE FAULT\n{:#?}\nError code: {:#x}",
        stack_frame, error_code
    );
    loop {}
}

extern "x86-interrupt" fn gp_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    error!(
        "EXCEPTION: #GP\n{:#?}\nError: {:#x}",
        stack_frame, error_code
    );
    loop {}
}

pub fn init_idt() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.general_protection_fault.set_handler_fn(gp_handler);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        IDT.load();
    }
    info!("IDT initialized and loaded.");
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

    info!("Double fault IST stack mapped and configured.");
}
