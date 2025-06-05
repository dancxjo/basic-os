#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    match rax {
        0x01 => write_port(rdi, rsi, rdx),
        0x02 => read_port(rdi),
        _ => {
            serial_println!("Unknown syscall: {:#x}", rax);
            !0
        }
    }
}

// Write to port (e.g., port 1 = console)
fn write_port(port: u64, data: u64, _flags: u64) -> u64 {
    match port {
        1 => {
            serial_print!("{}", data as u8 as char);
            0
        }
        _ => !0, // error
    }
}

// Read from port (e.g., port 2 = keyboard)
fn read_port(port: u64) -> u64 {
    match port {
        2 => {
            if let Some(byte) = crate::drivers::keyboard::pop_input() {
                byte as u64
            } else {
                !0 // No data available
            }
        }
        _ => !0,
    }
}

use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use crate::{serial_print, serial_println};
unsafe extern "C" {
    fn syscall_entry_asm(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64;
}
pub fn init_syscall() {
    use x86_64::VirtAddr;

    unsafe {
        // Enable syscall/sysret
        Efer::update(|efer| *efer |= EferFlags::SYSTEM_CALL_EXTENSIONS);

        // Set entry point for syscall
        LStar::write(VirtAddr::new(syscall_entry_asm as u64));

        // Define CS/SS selectors (CS for kernel, SS is unused by sysretq)
        let kernel_cs = 0x08u16;
        let user_cs = 0x1Bu16; // User code segment

        // Star::write now expects four arguments: kernel_cs, kernel_ss, user_cs, user_ss
        // kernel_ss is typically kernel_cs + 8, user_ss is typically user_cs + 8
        let kernel_ss = kernel_cs + 8;
        let user_ss = user_cs + 8;
        let _ = Star::write(
            x86_64::structures::gdt::SegmentSelector(kernel_cs),
            x86_64::structures::gdt::SegmentSelector(kernel_ss),
            x86_64::structures::gdt::SegmentSelector(user_cs),
            x86_64::structures::gdt::SegmentSelector(user_ss),
        );

        // Mask flags (e.g. disable interrupts during syscall entry)
        SFMask::write(x86_64::registers::rflags::RFlags::empty());
    }

    serial_println!("Syscall mechanism initialized.");
}
