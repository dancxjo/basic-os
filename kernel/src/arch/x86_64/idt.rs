#![allow(static_mut_refs)]

use crate::{
    arch::x86_64::interrupts::{end_of_interrupt, init_io_apic_irq},
    drivers::{keyboard::keyboard_interrupt_handler, mouse::mouse_interrupt_handler},
};
use core::sync::atomic::{AtomicU64, Ordering};
use log::error;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

// IST entries are 1-based in the IDT; we use slot 1 -> interrupt_stack_table[0].
pub const DOUBLE_FAULT_IST_INDEX: u16 = 1;

// === Internal ===

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

use x86_64::registers::control::Cr2;

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let faulting_address = Cr2::read();

    error!("\nEXCEPTION: PAGE FAULT");
    error!("Accessed Address: {:#018x}", faulting_address.as_u64());
    error!("Error Code: {:?}", error_code);
    error!("Stack Frame: {:#?}", stack_frame);

    if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
        error!("Cause: attempted WRITE");
    } else {
        error!("Cause: attempted READ");
    }

    if error_code.contains(PageFaultErrorCode::USER_MODE) {
        error!("From: USER MODE");
    } else {
        error!("From: KERNEL MODE");
    }

    if error_code.contains(PageFaultErrorCode::INSTRUCTION_FETCH) {
        error!("During: INSTRUCTION FETCH");
    }

    panic!("Unhandled page fault");
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
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);

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

#[cfg(feature = "threading")]
unsafe extern "C" {
    fn tick_handler(stack_frame: InterruptStackFrame);
}

/// Install device handlers (after memory is ready)
pub fn init_device_handlers() {
    unsafe {
        #[cfg(feature = "threading")]
        IDT[32].set_handler_fn(core::mem::transmute::<
            unsafe extern "C" fn(InterruptStackFrame),
            extern "x86-interrupt" fn(InterruptStackFrame),
        >(tick_handler));
        #[cfg(not(feature = "threading"))]
        IDT[32].set_handler_fn(timer_interrupt_handler); // Timer IRQ

        IDT[33].set_handler_fn(keyboard_interrupt_handler); // Keyboard IRQ
        IDT[44].set_handler_fn(mouse_interrupt_handler); // Mouse IRQ

        let bsp_apic_id = 0; // hardcoded for now, or read from the local APIC ID register

        init_io_apic_irq(1, 33, bsp_apic_id); // Keyboard
        init_io_apic_irq(12, 44, bsp_apic_id); // Mouse
    }
}
