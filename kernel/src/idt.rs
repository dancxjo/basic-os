//! idt.rs — ThingOS Interrupt Descriptor Table Setup (fault handlers only)

use core::mem::MaybeUninit;
use log::error;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::kthread;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

static mut IDT: MaybeUninit<InterruptDescriptorTable> = MaybeUninit::uninit();

// === Exception Handlers ===

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    error!(
        "Page fault! Error Code: {:?} Frame: {:?}",
        error_code, stack_frame
    );
    loop {}
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

// === Public API ===

/// Initialize the IDT with only the essential fault handlers.
pub fn init_idt() {
    unsafe {
        #[allow(static_mut_refs)]
        let idt = IDT.write(InterruptDescriptorTable::new());

        // Exception Handlers
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        install_basic_irq_handlers();
        idt.load();
        log::info!("Error-only IDT initialized and loaded statically.");
    }
}

extern "x86-interrupt" fn tick_handler(_stack_frame: InterruptStackFrame) {
    log::info!("Timer interrupt received.");
    kthread::schedule();
    crate::pic::pic_end_of_interrupt(0);
}

extern "x86-interrupt" fn dummy_keyboard_handler(_stack_frame: InterruptStackFrame) {
    log::info!("Keyboard interrupt received.");
    crate::pic::pic_end_of_interrupt(1);
}

pub fn install_basic_irq_handlers() {
    unsafe {
        #[allow(static_mut_refs)]
        let idt = IDT.assume_init_mut();

        idt[32].set_handler_fn(tick_handler);
        idt[33].set_handler_fn(dummy_keyboard_handler);
    }
}
