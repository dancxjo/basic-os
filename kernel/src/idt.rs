#![allow(static_mut_refs)]
use crate::serial_println;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
    unsafe {
        IDT.page_fault.set_handler_fn(page_fault_handler);
        IDT.double_fault.set_handler_fn(double_fault_handler);
        IDT.load();
    }
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
