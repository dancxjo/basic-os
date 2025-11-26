use uuid::Uuid;
use x86_64::VirtAddr;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use crate::serial_println;
use crate::telemetry::graph::{self, GraphFiatRequest, GraphThatRequest, WatchQuery};

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
        SYSCALL_GRAPH_FIAT => graph_fiat(rdi, rsi),
        SYSCALL_GRAPH_LINK => graph_link(rdi, rsi),
        SYSCALL_GRAPH_QUERY => graph_query(rdi, rsi),
        SYSCALL_GRAPH_GET => graph_get(rdi, rsi, rdx),
        SYSCALL_WATCH_REGISTER => watch_register(rdi, rsi),
        SYSCALL_WATCH_POLL => watch_poll(rdi, rsi, rdx),
        _ => {
            serial_println!("Unknown syscall: {:#x}", rax);
            !0
        }
    };
    ret
}

const SYSCALL_GRAPH_FIAT: u64 = 0x01;
const SYSCALL_GRAPH_LINK: u64 = 0x02;
const SYSCALL_GRAPH_QUERY: u64 = 0x03;
// const SYSCALL_GRAPH_WATCH: u64 = 0x04;
const SYSCALL_GRAPH_GET: u64 = 0x05;
const SYSCALL_WATCH_REGISTER: u64 = 0x06;
const SYSCALL_WATCH_POLL: u64 = 0x07;

fn graph_fiat(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GraphFiatRequest>(buf) else {
        return !0;
    };
    let thing = graph::fiat(request);
    thing.revision
}

fn graph_link(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GraphThatRequest>(buf) else {
        return !0;
    };
    graph::that(request)
}

fn graph_query(out_ptr: u64, out_len: u64) -> u64 {
    let bytes = match crate::telemetry::graph::export_snapshot_bytes() {
        Some(buf) => buf,
        None => return !0,
    };

    copy_out_slice(&bytes, out_ptr, out_len)
}

fn watch_register(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(query) = postcard::from_bytes::<WatchQuery>(buf) else {
        return !0;
    };
    graph::register_watch(query)
}

fn watch_poll(watch_id: u64, out_ptr: u64, out_len: u64) -> u64 {
    let bytes = match graph::export_watch_events(watch_id) {
        Some(buf) => buf,
        None => return !0,
    };
    copy_out_slice(&bytes, out_ptr, out_len)
}

fn graph_get(id_ptr: u64, out_ptr: u64, out_len: u64) -> u64 {
    if id_ptr == 0 {
        return !0;
    }
    let id_bytes = unsafe { core::slice::from_raw_parts(id_ptr as *const u8, 16) };
    let Ok(id) = Uuid::from_slice(id_bytes) else {
        return !0;
    };

    let bytes = match crate::telemetry::graph::export_thing_bytes(id) {
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
