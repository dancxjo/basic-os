use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use crate::drivers::irq_dma;
use crate::graph::{
    self, GrantCapabilityRequest, GraphFiatRequest, GraphFindByKind, GraphGetRequest,
    GraphLinkRequest, GraphPropsGetRequest, GraphPropsRequest, GraphThatRequest, NodePattern,
};
use thing_abi::WatchQuery;
use crate::serial_println;
use crate::task::runtime::current_bundle;

#[unsafe(no_mangle)]
static mut USER_RSP: u64 = 0;

#[unsafe(no_mangle)]
pub extern "C" fn syscall_entry(rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64) -> u64 {
    let ret = match rax {
        SYSCALL_GRAPH_FIAT => graph_fiat(rdi, rsi),
        SYSCALL_GRAPH_LINK => graph_link(rdi, rsi),
        // SYSCALL_GRAPH_QUERY => graph_query(rdi, rsi),
        SYSCALL_GRAPH_GET => graph_get(rdi, rsi, rdx, r10),
        SYSCALL_GRAPH_WATCH_REGISTER => watch_register(rdi, rsi),
        SYSCALL_GRAPH_WATCH_POLL => watch_poll(rdi, rsi, rdx),
        SYSCALL_KBD_READ => kbd_read(rdi, rsi),
        SYSCALL_FB_INFO => fb_info(rdi, rsi),
        SYSCALL_FB_MAP => fb_map(),
        SYSCALL_GRAPH_FIND_BY_KIND => graph_find_by_kind(rdi, rsi, rdx),
        SYSCALL_MOUSE_READ => mouse_read(rdi, rsi),
        SYSCALL_GRAPH_GET_PROPS => graph_get_props(rdi, rsi, rdx, r10),
        SYSCALL_GRAPH_SET_PROPS => graph_set_props(rdi, rsi),
        SYSCALL_GRANT_CAPABILITY => grant_capability(rdi, rsi),
        SYSCALL_IRQ_BIND => irq_bind(rdi, rsi),
        SYSCALL_IRQ_ACK => irq_ack(rdi),
        SYSCALL_DMA_MAP => dma_map(rdi, rsi),
        SYSCALL_DMA_SUBMIT => dma_submit(rdi, rsi),
        SYSCALL_DMA_WAIT => dma_wait(rdi, rsi),
        SYSCALL_DEV_OPEN => dev_open(rdi, rsi),
        SYSCALL_DEV_READ => dev_read(rdi, rsi, rdx),
        SYSCALL_DEV_WRITE => dev_write(rdi, rsi, rdx),
        SYSCALL_DEV_MAP => dev_map(rdi),
        SYSCALL_LOG => sys_log(rdi, rsi),
        _ => {
            serial_println!("Unknown syscall: {:#x}", rax);
            !0
        }
    };
    ret
}

const SYSCALL_GRAPH_FIAT: u64 = 0x01;
const SYSCALL_GRAPH_LINK: u64 = 0x02;
// const SYSCALL_GRAPH_QUERY: u64 = 0x03;
// const SYSCALL_GRAPH_WATCH: u64 = 0x04;
const SYSCALL_GRAPH_GET: u64 = 0x05;
const SYSCALL_GRAPH_WATCH_REGISTER: u64 = 0x06;
const SYSCALL_GRAPH_WATCH_POLL: u64 = 0x07;
const SYSCALL_KBD_READ: u64 = 0x08;
const SYSCALL_FB_INFO: u64 = 0x09;
const SYSCALL_FB_MAP: u64 = 0x0A;
const SYSCALL_GRAPH_FIND_BY_KIND: u64 = 0x0B;
const SYSCALL_MOUSE_READ: u64 = 0x0C;
const SYSCALL_GRAPH_GET_PROPS: u64 = 0x0E;
const SYSCALL_GRAPH_SET_PROPS: u64 = 0x0F;
const SYSCALL_GRANT_CAPABILITY: u64 = 0x10;
const SYSCALL_IRQ_BIND: u64 = 0x11;
const SYSCALL_IRQ_ACK: u64 = 0x12;
const SYSCALL_DMA_MAP: u64 = 0x13;
const SYSCALL_DMA_SUBMIT: u64 = 0x14;
const SYSCALL_DMA_WAIT: u64 = 0x15;
const SYSCALL_DEV_OPEN: u64 = 0x20;
const SYSCALL_DEV_READ: u64 = 0x21;
const SYSCALL_DEV_WRITE: u64 = 0x22;
const SYSCALL_DEV_MAP: u64 = 0x23;
const SYSCALL_LOG: u64 = 0x99;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrqBindRequest {
    pub device: Uuid,
    pub irq_line: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmaMapRequest {
    pub buffer: Uuid,
    pub flags: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmaSubmitRequest {
    pub device: Uuid,
    pub mapping: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmaWaitRequest {
    pub handle: u64,
    pub timeout_ms: u64,
}

fn graph_fiat(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    // crate::serial_println!("graph_fiat: ptr={:#x} len={}", req_ptr, req_len);
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };

    // crate::serial_println!("graph_fiat: skipping postcard");

    let Ok(request) = postcard::from_bytes::<GraphFiatRequest>(buf) else {
        crate::serial_println!("graph_fiat: postcard failed");
        return !0;
    };

    // crate::serial_println!("graph_fiat: postcard success");
    // return 0; // Fake success

    let bundle = current_bundle();
    let thing = graph::fiat_for_bundle(bundle, request);
    thing.revision
}

fn graph_link(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GraphLinkRequest>(buf) else {
        return !0;
    };
    let bundle = current_bundle();
    graph::link(bundle, request)
}

fn watch_register(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    if let Ok(pattern) = postcard::from_bytes::<NodePattern>(buf) {
        return graph::register_watch_pattern(current_bundle(), pattern);
    }
    let Ok(query) = postcard::from_bytes::<WatchQuery>(buf) else {
        return !0;
    };
    graph::register_watch(current_bundle(), query)
}

fn watch_poll(watch_id: u64, out_ptr: u64, out_len: u64) -> u64 {
    let bytes = match graph::export_watch_events(watch_id) {
        Some(buf) => buf,
        None => return !0,
    };
    // serial_println!("watch_poll: id={} out_ptr={:#x} out_len={} bytes_len={}", watch_id, out_ptr, out_len, bytes.len());
    copy_out_slice(&bytes, out_ptr, out_len)
}

fn kbd_read(out_ptr: u64, out_len: u64) -> u64 {
    if out_ptr == 0 || out_len == 0 || out_ptr >= 0x0000_8000_0000_0000 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, out_len as usize) };
    crate::drivers::keyboard::read_keyboard(buf) as u64
}

fn mouse_read(out_ptr: u64, out_len: u64) -> u64 {
    if out_ptr == 0 || out_len == 0 || out_ptr >= 0x0000_8000_0000_0000 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, out_len as usize) };
    crate::drivers::mouse::read_mouse(buf) as u64
}

fn graph_get(req_ptr: u64, req_len: u64, out_ptr: u64, out_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let request = if let Ok(req) = postcard::from_bytes::<GraphGetRequest>(buf) {
        Some(req)
    } else if req_len as usize == 16 {
        Uuid::from_slice(buf).ok().map(GraphGetRequest::Thing)
    } else {
        None
    };

    let Some(request) = request else {
        return !0;
    };

    match request {
        GraphGetRequest::Thing(id) => {
            let bytes = match crate::graph::export_thing_bytes(current_bundle(), id) {
                Some(buf) => buf,
                None => return !0,
            };

            copy_out_slice(&bytes, out_ptr, out_len)
        }
        GraphGetRequest::Pattern(pattern) => {
            let nodes = graph::get_nodes(current_bundle(), pattern);
            let bytes = match postcard::to_allocvec(&nodes) {
                Ok(b) => b,
                Err(_) => return !0,
            };
            copy_out_slice(&bytes, out_ptr, out_len)
        }
    }
}

fn copy_out_slice(buf: &[u8], out_ptr: u64, out_len: u64) -> u64 {
    let required = buf.len() as u64;
    if out_len == 0 || out_ptr == 0 || out_ptr >= 0x0000_8000_0000_0000 {
        return required;
    }

    if out_len < required {
        return required;
    }

    // serial_println!("copy_out: src={:p} dst={:#x} len={}", buf.as_ptr(), out_ptr, buf.len());
    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), out_ptr as *mut u8, buf.len());
    }
    required
}

fn fb_info(out_ptr: u64, out_len: u64) -> u64 {
    if out_ptr == 0 || out_len == 0 || out_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let info = match crate::drivers::framebuffer::get_framebuffer_info() {
        Some(info) => info,
        None => return !0,
    };

    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const _ as *const u8,
            core::mem::size_of::<crate::drivers::framebuffer::FramebufferInfo>(),
        )
    };

    copy_out_slice(bytes, out_ptr, out_len)
}

fn fb_map() -> u64 {
    match crate::drivers::framebuffer::get_framebuffer_info() {
        Some(info) => info.addr,
        None => 0,
    }
}

fn graph_find_by_kind(req_ptr: u64, out_ptr: u64, out_len: u64) -> u64 {
    if req_ptr == 0
        || out_ptr == 0
        || out_len == 0
        || req_ptr >= 0x0000_8000_0000_0000
        || out_ptr >= 0x0000_8000_0000_0000
    {
        return !0;
    }

    let req_buf = unsafe {
        core::slice::from_raw_parts(
            req_ptr as *const u8,
            core::mem::size_of::<GraphFindByKind>(),
        )
    };
    let Ok(request) = postcard::from_bytes::<GraphFindByKind>(req_buf) else {
        return !0;
    };

    let kind_str = unsafe {
        let slice =
            core::slice::from_raw_parts(request.kind_ptr as *const u8, request.kind_len as usize);
        core::str::from_utf8_unchecked(slice)
    };

    let bundle = current_bundle();
    let bytes = match crate::graph::export_find_by_kind_bytes(bundle, kind_str, request.cursor) {
        Some(buf) => buf,
        None => return !0,
    };

    copy_out_slice(&bytes, out_ptr, out_len)
}

fn graph_get_props(req_ptr: u64, req_len: u64, out_ptr: u64, out_len: u64) -> u64 {
    if req_ptr == 0 || req_ptr >= 0x0000_8000_0000_0000 || out_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GraphPropsGetRequest>(buf) else {
        return !0;
    };
    let Some(map) = graph::get_props(current_bundle(), request) else {
        return !0;
    };
    let bytes = match postcard::to_allocvec(&map) {
        Ok(b) => b,
        Err(_) => return !0,
    };
    copy_out_slice(&bytes, out_ptr, out_len)
}

fn graph_set_props(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GraphPropsRequest>(buf) else {
        return !0;
    };
    if graph::set_props(current_bundle(), request) {
        0
    } else {
        !0
    }
}

fn grant_capability(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<GrantCapabilityRequest>(buf) else {
        return !0;
    };
    if graph::grant_capability(current_bundle(), request) {
        0
    } else {
        !0
    }
}

fn irq_bind(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<IrqBindRequest>(buf) else {
        return !0;
    };
    irq_dma::irq_bind(current_bundle(), request.device, request.irq_line).unwrap_or(!0)
}

fn irq_ack(handle: u64) -> u64 {
    if irq_dma::irq_ack(current_bundle(), handle) {
        0
    } else {
        !0
    }
}

fn dma_map(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<DmaMapRequest>(buf) else {
        return !0;
    };
    irq_dma::dma_map(current_bundle(), request.buffer, request.flags).unwrap_or(!0)
}

fn dma_submit(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<DmaSubmitRequest>(buf) else {
        return !0;
    };
    irq_dma::dma_submit(
        current_bundle(),
        request.device,
        request.mapping,
        request.bytes,
    )
    .unwrap_or(!0)
}

fn dma_wait(req_ptr: u64, req_len: u64) -> u64 {
    if req_ptr == 0 || req_len == 0 || req_ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    let buf = unsafe { core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };
    let Ok(request) = postcard::from_bytes::<DmaWaitRequest>(buf) else {
        return !0;
    };
    if irq_dma::dma_wait(current_bundle(), request.handle).is_some() {
        0
    } else {
        !0
    }
}

fn sys_log(ptr: u64, len: u64) -> u64 {
    if ptr == 0 || len == 0 || len > 4096 {
        return !0;
    }
    // Check user range
    if ptr >= 0x0000_8000_0000_0000 {
        return !0;
    }
    if ptr + len >= 0x0000_8000_0000_0000 {
        return !0;
    }

    let buf = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };
    if let Ok(s) = core::str::from_utf8(buf) {
        crate::serial_print!("{}", s);
        return 0;
    }
    !0
}

fn dev_open(kind: u64, index: u64) -> u64 {
    crate::drivers::device::dev_open(kind as u32, index as usize).unwrap_or(!0)
}

fn dev_read(handle: u64, out_ptr: u64, out_len: u64) -> u64 {
    if out_ptr == 0 || out_len == 0 || out_ptr >= 0x0000_8000_0000_0000 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, out_len as usize) };
    crate::drivers::device::dev_read(handle, buf) as u64
}

fn dev_write(handle: u64, in_ptr: u64, in_len: u64) -> u64 {
    if in_ptr == 0 || in_len == 0 || in_ptr >= 0x0000_8000_0000_0000 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts(in_ptr as *const u8, in_len as usize) };
    crate::drivers::device::dev_write(handle, buf) as u64
}

fn dev_map(handle: u64) -> u64 {
    match crate::drivers::device::dev_map(handle) {
        Some((addr, _len)) => addr,
        None => 0,
    }
}

unsafe extern "C" {
    fn syscall_entry_asm(rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64) -> u64;
}
pub fn init_syscall() {
    use x86_64::VirtAddr;

    unsafe {
        // Enable syscall/sysret
        Efer::update(|efer| *efer |= EferFlags::SYSTEM_CALL_EXTENSIONS);

        // Set entry point for syscall
        LStar::write(VirtAddr::new(syscall_entry_asm as *const () as u64));

        // Define CS/SS selectors (CS for kernel, SS is unused by sysretq)
        let _kernel_cs = 0x08u16;
        // For sysretq, we need a base selector such that:
        // CS = base + 16 (0x10)
        // SS = base + 8  (0x08)
        // Our GDT has UserData at 0x28 (Index 5) and UserCode at 0x30 (Index 6).
        // So we need base + 8 = 0x28 => base = 0x20.
        // We use RPL 3 for user segments, so 0x20 | 3 = 0x23.
        let _user_cs = 0x23u16;

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
