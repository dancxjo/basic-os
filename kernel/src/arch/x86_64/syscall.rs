use alloc::string::{String, ToString};
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use crate::telemetry::{
    canon,
    canon::Symbol,
    journal::{self, Event, Value},
};
use crate::{serial_print, serial_println};

#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    match rax {
        SYSCALL_WRITE_PORT => write_port(rdi, rsi, rdx),
        SYSCALL_READ_PORT => read_port(rdi),
        SYSCALL_JOURNAL_EMIT => journal_emit(rdi, rsi, rdx),
        SYSCALL_JOURNAL_SNAPSHOT => journal_snapshot(rdi, rsi),
        SYSCALL_GRAPH_SNAPSHOT => graph_snapshot(rdi, rsi),
        _ => {
            serial_println!("Unknown syscall: {:#x}", rax);
            !0
        }
    }
}

const SYSCALL_WRITE_PORT: u64 = 0x01;
const SYSCALL_READ_PORT: u64 = 0x02;
const SYSCALL_JOURNAL_EMIT: u64 = 0x10;
const SYSCALL_JOURNAL_SNAPSHOT: u64 = 0x11;
const SYSCALL_GRAPH_SNAPSHOT: u64 = 0x12;

// Write to port (e.g., port 1 = console)
fn write_port(port: u64, data: u64, _flags: u64) -> u64 {
    match port {
        1 => {
            crate::drivers::framebuffer::console_write_byte(data as u8);
            serial_print!("{}", data as u8 as char);
            0
        }
        _ => !0, // error
    }
}

fn journal_emit(kind_raw: u64, data_ptr: u64, len: u64) -> u64 {
    if data_ptr == 0 && len > 0 {
        return !0;
    }

    let kind = Symbol::new(kind_raw as u32);
    let data_slice = unsafe { core::slice::from_raw_parts(data_ptr as *const u8, len as usize) };

    let data = postcard::from_bytes::<Value>(data_slice).unwrap_or_else(|_| {
        match core::str::from_utf8(data_slice) {
            Ok(s) => Value::Text(s.into()),
            Err(_) => Value::Bytes(data_slice.to_vec()),
        }
    });

    let event = Event::new(kind, data);
    let _ = journal::emit(event.clone());
    reflect_write_event(&event);
    0
}

fn journal_snapshot(out_ptr: u64, out_len: u64) -> u64 {
    let bytes = match journal::export_bytes() {
        Some(buf) => buf,
        None => return !0,
    };

    copy_out_slice(&bytes, out_ptr, out_len)
}

fn graph_snapshot(out_ptr: u64, out_len: u64) -> u64 {
    let bytes = match crate::telemetry::graph::export_snapshot_bytes() {
        Some(buf) => buf,
        None => return !0,
    };

    copy_out_slice(&bytes, out_ptr, out_len)
}

fn copy_out_slice(buf: &[u8], out_ptr: u64, out_len: u64) -> u64 {
    let required = buf.len() as u64;
    if out_len == 0 || out_ptr == 0 {
        return required;
    }

    if out_len < required {
        return required;
    }

    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), out_ptr as *mut u8, buf.len());
    }
    required
}

fn reflect_write_event(event: &Event) {
    if event.kind != canon::WRITE && event.kind != canon::FRAME_READY {
        return;
    }

    if let Some(text) = extract_text(&event.data) {
        for byte in text.bytes() {
            crate::drivers::framebuffer::console_write_byte(byte);
        }
    }
}

fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Bytes(b) => core::str::from_utf8(b).ok().map(|s| s.to_string()),
        Value::Map(m) => m.get(&canon::TEXT).and_then(extract_text),
        _ => None,
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
