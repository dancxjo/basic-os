//! idt.rs — ThingOS Interrupt Descriptor Table Setup (fault handlers only)

use core::mem::MaybeUninit;
use log::{error, info};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

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

unsafe extern "C" {
    fn tick_handler();
}

pub fn install_basic_irq_handlers() {
    #[allow(static_mut_refs)]
    let idt = unsafe { IDT.assume_init_mut() };
    unsafe {
        idt[32].set_handler_addr(core::mem::transmute(tick_handler as *const ()));
    }
}
