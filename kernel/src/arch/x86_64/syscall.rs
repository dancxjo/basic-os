use alloc::string::{String, ToString};
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use crate::drivers::device;
use crate::telemetry::{
    canon,
    canon::Symbol,
    graph::{self, GraphFiatRequest, GraphThatRequest},
    journal::{self, Event, Value},
};
use crate::{serial_print, serial_println};

#[unsafe(no_mangle)]
static mut USER_RSP: u64 = 0;

const KERNEL_STACK_SIZE: usize = 16 * 1024; // 16 KiB

#[repr(C, align(16))]
struct AlignedStack([u8; KERNEL_STACK_SIZE]);

#[unsafe(no_mangle)]
static mut SYSCALL_KERNEL_STACK: AlignedStack = AlignedStack([0; KERNEL_STACK_SIZE]);

#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    let ret = match rax {
        SYSCALL_WRITE_PORT => write_port(rdi, rsi, rdx),
        SYSCALL_READ_PORT => read_port(rdi),
        SYSCALL_JOURNAL_EMIT => journal_emit(rdi, rsi, rdx),
        SYSCALL_JOURNAL_SNAPSHOT => journal_snapshot(rdi, rsi),
        SYSCALL_GRAPH_SNAPSHOT => graph_snapshot(rdi, rsi),
        SYSCALL_DEV_OPEN => dev_open(rdi as u32, rsi as usize),
        SYSCALL_DEV_READ => dev_read(rdi, rsi, rdx),
        SYSCALL_DEV_WRITE => dev_write(rdi, rsi, rdx),
        SYSCALL_DEV_MAP => dev_map(rdi, rsi, rdx),
        _ => {
            serial_println!("Unknown syscall: {:#x}", rax);
            !0
        }
    };
    ret
}

const SYSCALL_WRITE_PORT: u64 = 0x01;
const SYSCALL_READ_PORT: u64 = 0x02;
const SYSCALL_JOURNAL_EMIT: u64 = 0x10;
const SYSCALL_JOURNAL_SNAPSHOT: u64 = 0x11;
const SYSCALL_GRAPH_SNAPSHOT: u64 = 0x12;
const SYSCALL_DEV_OPEN: u64 = 0x20;
const SYSCALL_DEV_READ: u64 = 0x21;
const SYSCALL_DEV_WRITE: u64 = 0x22;
const SYSCALL_DEV_MAP: u64 = 0x23;

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
    serial_println!(
        "journal_emit: kind={:#x} ptr={:#x} len={}",
        kind_raw,
        data_ptr,
        len
    );
    if data_ptr == 0 && len > 0 {
        return !0;
    }

    // Test allocator
    {
        let mut test_vec = alloc::vec::Vec::new();
        test_vec.push(1u8);
        serial_println!("Allocator test: vec len = {}", test_vec.len());
    }

    let kind = Symbol::new(kind_raw as u32);
    let data_slice = unsafe { core::slice::from_raw_parts(data_ptr as *const u8, len as usize) };

    serial_println!("journal_emit: parsing data...");
    // Try to read the slice first to ensure it's accessible
    let mut sum: u64 = 0;
    for b in data_slice {
        sum += *b as u64;
    }
    serial_println!("journal_emit: slice sum = {}", sum);

    let data = postcard::from_bytes::<Value>(data_slice).unwrap_or_else(|_| {
        match core::str::from_utf8(data_slice) {
            Ok(s) => Value::Text(s.into()),
            Err(_) => Value::Bytes(data_slice.to_vec()),
        }
    });
    serial_println!("journal_emit: data parsed. Creating event...");

    let event = Event::new(kind, data);
    serial_println!("journal_emit: event created. Emitting...");
    apply_graph_side_effect(&event);
    let _ = journal::emit(event.clone());
    reflect_write_event(&event);
    serial_println!("journal_emit: done.");
    0
}

fn apply_graph_side_effect(event: &Event) {
    if let Some(map) = event.data.as_map() {
        if event.kind == canon::THING_CREATED {
            if let Some(req) = build_graph_fiat(map) {
                let _ = graph::fiat(req);
            }
        } else if event.kind == canon::EDGE_ADDED {
            if let Some(req) = build_graph_that(map) {
                let _ = graph::that(req);
            }
        }
    }
}

fn build_graph_fiat(fields: &BTreeMap<Symbol, Value>) -> Option<GraphFiatRequest> {
    let id = fields.get(&canon::ID).and_then(Value::as_uuid);
    let kind = fields.get(&canon::KIND).and_then(Value::as_symbol)?;
    let raw_fields = fields.get(&canon::FIELDS).and_then(Value::as_map)?.clone();
    Some(GraphFiatRequest {
        id,
        kind,
        fields: raw_fields,
    })
}

fn build_graph_that(fields: &BTreeMap<Symbol, Value>) -> Option<GraphThatRequest> {
    let src = fields.get(&canon::SRC).and_then(Value::as_uuid)?;
    let dst = fields.get(&canon::DST).and_then(Value::as_uuid)?;
    let pred = fields.get(&canon::PREDICATE).and_then(Value::as_symbol)?;
    let revision_hint = fields
        .get(&canon::REVISION)
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(GraphThatRequest {
        src,
        pred,
        dst,
        revision_hint,
    })
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

fn dev_open(kind_raw: u32, index: usize) -> u64 {
    match device::dev_open(kind_raw, index) {
        Some(handle) => handle,
        None => !0,
    }
}

fn dev_read(handle: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len as usize) };
    device::dev_read(handle, buf) as u64
}

fn dev_write(handle: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len as usize) };
    device::dev_write(handle, buf) as u64
}

fn dev_map(handle: u64, len_out_ptr: u64, _hint: u64) -> u64 {
    let Some((addr, len)) = device::dev_map(handle) else {
        return 0;
    };
    if len_out_ptr != 0 {
        unsafe {
            *(len_out_ptr as *mut u64) = len as u64;
        }
    }
    addr
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
        LStar::write(VirtAddr::new(syscall_entry_asm as *const () as u64));

        // Define CS/SS selectors (CS for kernel, SS is unused by sysretq)
        let kernel_cs = 0x08u16;
        // For sysretq, we need a base selector such that:
        // CS = base + 16 (0x10)
        // SS = base + 8  (0x08)
        // Our GDT has UserData at 0x28 (Index 5) and UserCode at 0x30 (Index 6).
        // So we need base + 8 = 0x28 => base = 0x20.
        // We use RPL 3 for user segments, so 0x20 | 3 = 0x23.
        let user_cs = 0x23u16;

        // Star::write expects: cs_sysret, ss_sysret, cs_syscall, ss_syscall
        // cs_sysret: User Code (0x33 = 0x30 | 3)
        // ss_sysret: User Data (0x2b = 0x28 | 3)
        // cs_syscall: Kernel Code (0x08)
        // ss_syscall: Kernel Data (0x10)

        let kernel_cs = x86_64::structures::gdt::SegmentSelector(0x08);
        let kernel_ss = x86_64::structures::gdt::SegmentSelector(0x10);
        let user_cs = x86_64::structures::gdt::SegmentSelector(0x30 | 3);
        let user_ss = x86_64::structures::gdt::SegmentSelector(0x28 | 3);

        Star::write(user_cs, user_ss, kernel_cs, kernel_ss).expect("Failed to set STAR MSR");

        // Mask flags (e.g. disable interrupts during syscall entry)
        SFMask::write(x86_64::registers::rflags::RFlags::INTERRUPT_FLAG);
    }

    serial_println!("Syscall mechanism initialized.");
}
