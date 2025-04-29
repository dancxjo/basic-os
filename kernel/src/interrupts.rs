// interrupts.rs — APIC-based interrupt management for ThingOS

use core::arch::asm;

use log::info;
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;

use crate::pic::{init_pic, pic_end_of_interrupt};
use crate::{bootstrap_step, pic};
use x86_64::registers::control::{Cr2, Cr3};
use x86_64::registers::rflags::RFlags;

// IA32_APIC_BASE MSR address (0x1B)
const IA32_APIC_BASE: u32 = 0x1B;

const APIC_BASE: usize = 0xFEE00000; // Default LAPIC memory-mapped base address

/// Disable the legacy 8259 PIC completely
pub fn disable_pic() {
    unsafe {
        let mut pic1_data = Port::new(0x21);
        let mut pic2_data = Port::new(0xA1);

        pic1_data.write(0xFFu8); // Mask all interrupts on master PIC
        pic2_data.write(0xFFu8); // Mask all interrupts on slave PIC
    }
    log::info!("Legacy PIC disabled.");
}

/// Enable the CPU's Local APIC by setting the IA32_APIC_BASE MSR
pub fn enable_apic() {
    unsafe {
        let mut apic_base = Msr::new(IA32_APIC_BASE);
        let mut value = apic_base.read();
        value |= 1 << 11; // Set the enable bit (bit 11)
        apic_base.write(value);

        // ALSO: Program the Spurious Interrupt Vector Register
        const SPURIOUS_VECTOR_REGISTER: usize = 0xF0;
        const APIC_BASE: usize = 0xFEE00000;
        const SPURIOUS_VECTOR: u8 = 0xFF; // (Any valid vector > 32)

        core::ptr::write_volatile(
            (APIC_BASE + SPURIOUS_VECTOR_REGISTER) as *mut u32,
            (SPURIOUS_VECTOR as u32) | (1 << 8), // Set spurious vector and enable LAPIC
        );
    }
    log::info!("Local APIC enabled (spurious vector 0xFF).");
}

/// Write to a Local APIC register
fn apic_write(offset: usize, value: u32) {
    unsafe { core::ptr::write_volatile((APIC_BASE + offset) as *mut u32, value) }
}

/// Read from a Local APIC register
fn apic_read(offset: usize) -> u32 {
    unsafe { core::ptr::read_volatile((APIC_BASE + offset) as *mut u32) }
}

/// Set up the Local APIC timer to fire periodic interrupts
pub fn setup_apic_timer() {
    // Set divide configuration (Divide by 1)
    apic_write(0x3E0, 0b1011);

    // Set the Local Vector Table (LVT) Timer entry
    // 0x20020: Vector 32 (0x20), periodic mode
    apic_write(0x320, 0x20020);

    // Set initial count (this value controls frequency)
    apic_write(0x380, 100_000);
    log::info!("Local APIC Timer configured.");
}

pub fn init_interrupts() {
    #[cfg(feature = "apic")]
    {
        init_apic();
    }

    #[cfg(not(feature = "apic"))]
    {
        unsafe { init_pic() };
    }

    // Only now enable interrupts
    let cr2 = Cr2::read();
    info!("CR2 (faulting address): {:#x}", cr2);

    let cr3 = Cr3::read();
    info!(
        "CR3 (page table base): {:#x}",
        cr3.0.start_address().as_u64()
    );

    let rflags = x86_64::registers::rflags::read();
    info!("RFLAGS: {:#x}", rflags.bits());

    x86_64::instructions::interrupts::enable(); // Only one sti here
}

pub fn init_apic() {
    disable_pic();
    enable_apic();
    setup_apic_timer();
}

// /// Switch to the newly mapped kernel stack before enabling interrupts
// pub unsafe fn switch_to_kernel_stack() {
//     info!("Switching to safe kernel stack at {:#x}", STACK_TOP);
//     unsafe {
//         asm!(
//             "mov rsp, {0}",
//             in(reg) STACK_TOP,
//             options(nostack)
//         )
//     };
//     info!("2 Kernel stack switched to {:#x}", STACK_TOP);
//     // loop {}
// }

/// Notify Local APIC that interrupt processing is complete
fn lapic_end_of_interrupt() {
    unsafe {
        const APIC_EOI_REGISTER: usize = 0xB0;
        const APIC_BASE: usize = 0xFEE00000;
        core::ptr::write_volatile((APIC_BASE + APIC_EOI_REGISTER) as *mut u32, 0);
    }
}

pub fn end_of_interrupt() {
    #[cfg(feature = "apic")]
    {
        lapic_end_of_interrupt();
    }

    #[cfg(not(feature = "apic"))]
    {
        pic_end_of_interrupt(0);
    }
}
