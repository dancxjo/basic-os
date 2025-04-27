//! idt.rs — Early and Late IDT setup for ThingOS

#![allow(static_mut_refs)]

use crate::interrupts::lapic_end_of_interrupt;
use x86_64::VirtAddr;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// === Internal ===

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: PageFaultErrorCode,
) {
    log::error!("Page fault occurred! {:?}: {:?}", _error_code, _stack_frame);
    loop {}
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    log::error!("Double fault occurred!");
    loop {}
}

extern "x86-interrupt" fn general_protection_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    log::error!("General protection fault occurred!");
    loop {}
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    lapic_end_of_interrupt();
}

extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    lapic_end_of_interrupt();
}

// === Public API ===

/// Install only fault handlers (no device IRQs yet)
pub fn init_fault_handlers() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        IDT.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        IDT.load();
    }
    log::info!("Fault handlers initialized (PageFault, DoubleFault, GPFault).");
}

/// Install device handlers (after memory is ready)
pub fn init_device_handlers() {
    unsafe {
        IDT[32].set_handler_fn(timer_interrupt_handler); // Timer IRQ
        IDT[0xFF].set_handler_fn(spurious_interrupt_handler); // Spurious
        IDT[0xFE].set_handler_fn(spurious_interrupt_handler); // Another spurious
        IDT.load(); // reload IDT after updates
    }
    log::info!("Device interrupt handlers initialized (Timer, Spurious).");
}
