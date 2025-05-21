#![allow(static_mut_refs)]

use crate::{
    input::{keyboard_interrupt_handler, mouse_interrupt_handler},
    interrupts::{end_of_interrupt, init_io_apic_irq},
};
use core::sync::atomic::{AtomicU64, Ordering};
use log::{error, info};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// === Internal ===

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    error!(
        "Page fault! Error Code: {:?} Frame: {:?}",
        error_code, stack_frame
    );
    loop {
        info!("Page fault handler called");
    }
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    error!("Double fault! Frame: {:?}", stack_frame);
    loop {}
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    error!(
        "General protection fault! Code: {:?} Frame: {:?}",
        error_code, stack_frame
    );
    loop {}
}

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let ticks = TICK_COUNT.fetch_add(1, Ordering::Relaxed);

    // if ticks % (100_000 / 60) == 0 {
    //     // log::info!("Tick count: {}", ticks);
    // }

    end_of_interrupt(0);
}

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
