#![allow(static_mut_refs)]

use crate::{
    input::{keyboard_interrupt_handler, mouse_interrupt_handler},
    interrupts::{end_of_interrupt, init_io_apic_irq},
};
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

use core::sync::atomic::{AtomicU64, Ordering};

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let ticks = TICK_COUNT.fetch_add(1, Ordering::Relaxed);

    if ticks % (100_000 / 60) == 0 {
        // log::info!("Tick count: {}", ticks);
    }

    end_of_interrupt(0);
}

/// Install only fault handlers (no device IRQs yet)
pub fn init_idt() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        IDT.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        init_device_handlers();
        IDT.load();
    }
    log::info!("Fault handlers initialized (PageFault, DoubleFault, GPFault).");
}

/// Install device handlers (after memory is ready)
pub fn init_device_handlers() {
    unsafe {
        IDT[32].set_handler_fn(timer_interrupt_handler); // Timer IRQ
        IDT[33].set_handler_fn(keyboard_interrupt_handler); // Keyboard IRQ
        IDT[44].set_handler_fn(mouse_interrupt_handler); // Mouse IRQ

        let bsp_apic_id = 0; // hardcoded for now, or read from the local APIC ID register

        init_io_apic_irq(1, 33, bsp_apic_id); // Keyboard
        init_io_apic_irq(12, 44, bsp_apic_id); // Mouse
    }
}
