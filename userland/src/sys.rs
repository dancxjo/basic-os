use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw syscall entry point (rax, rdi, rsi, rdx, r10).
#[inline(always)]
pub unsafe fn syscall(rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") rax => ret,
        inout("rdi") rdi => _,
        inout("rsi") rsi => _,
        inout("rdx") rdx => _,
        in("r10") r10,
        out("rcx") _,
        out("r11") _,
        out("r8") _,
        out("r9") _,
        lateout("r10") _,
        out("xmm0") _,
        out("xmm1") _,
        out("xmm2") _,
        out("xmm3") _,
        out("xmm4") _,
        out("xmm5") _,
        out("xmm6") _,
        out("xmm7") _,
        out("xmm8") _,
        out("xmm9") _,
        out("xmm10") _,
        out("xmm11") _,
        out("xmm12") _,
        out("xmm13") _,
        out("xmm14") _,
        out("xmm15") _,
        options(nostack)
    );
    ret
}

pub const SYSCALL_GRAPH_FIAT: u64 = 0x01;
pub const SYSCALL_GRAPH_LINK: u64 = 0x02;
// pub const SYSCALL_GRAPH_QUERY: u64 = 0x03;
// pub const SYSCALL_GRAPH_WATCH: u64 = 0x04;
pub const SYSCALL_GRAPH_GET: u64 = 0x05;
pub const SYSCALL_GRAPH_WATCH_REGISTER: u64 = 0x06;
pub const SYSCALL_GRAPH_WATCH_POLL: u64 = 0x07;
pub const SYSCALL_KBD_READ: u64 = 0x08;
pub const SYSCALL_FB_INFO: u64 = 0x09;
pub const SYSCALL_FB_MAP: u64 = 0x0A;
pub const SYSCALL_GRAPH_FIND_BY_KIND: u64 = 0x0B;
pub const SYSCALL_MOUSE_READ: u64 = 0x0C;
pub const SYSCALL_GRAPH_GET_PROPS: u64 = 0x0E;
pub const SYSCALL_GRAPH_SET_PROPS: u64 = 0x0F;
pub const SYSCALL_GRANT_CAPABILITY: u64 = 0x10;
pub const SYSCALL_IRQ_BIND: u64 = 0x11;
pub const SYSCALL_IRQ_ACK: u64 = 0x12;
pub const SYSCALL_DMA_MAP: u64 = 0x13;
pub const SYSCALL_DMA_SUBMIT: u64 = 0x14;
pub const SYSCALL_DMA_WAIT: u64 = 0x15;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[repr(C)]
pub struct GraphFindByKind {
    pub kind_ptr: u64,
    pub kind_len: u64,
    pub cursor: u64,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[repr(C)]
pub struct GraphFindResultHeader {
    pub next_cursor: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IrqBindRequest {
    pub device: Uuid,
    pub irq_line: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DmaMapRequest {
    pub buffer: Uuid,
    pub flags: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DmaSubmitRequest {
    pub device: Uuid,
    pub mapping: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DmaWaitRequest {
    pub handle: u64,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInfo {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u64,
    pub addr: u64,
}

pub fn fb_info() -> Option<FramebufferInfo> {
    let mut info = FramebufferInfo {
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
        addr: 0,
    };
    let ret = unsafe {
        syscall(
            SYSCALL_FB_INFO,
            &mut info as *mut _ as u64,
            core::mem::size_of::<FramebufferInfo>() as u64,
            0,
            0,
        )
    };
    if ret == 0 {
        Some(info)
    } else {
        None
    }
}

pub fn fb_map() -> u64 {
    unsafe { syscall(SYSCALL_FB_MAP, 0, 0, 0, 0) }
}

pub fn graph_fiat_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_FIAT,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    }
}

pub fn graph_find_by_kind_raw(req: &GraphFindByKind, out: &mut [u8]) -> u64 {
    let req_ptr = req as *const _ as u64;
    unsafe {
        syscall(
            SYSCALL_GRAPH_FIND_BY_KIND,
            req_ptr,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub fn graph_link_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_LINK,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    }
}

pub fn graph_get_props_raw(request: &[u8], out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_GET_PROPS,
            request.as_ptr() as u64,
            request.len() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

pub fn graph_set_props_raw(request: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_SET_PROPS,
            request.as_ptr() as u64,
            request.len() as u64,
            0,
            0,
        )
    }
}

pub fn graph_watch_register_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_WATCH_REGISTER,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    }
}

pub fn graph_watch_poll_raw(watch_id: u64, out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_WATCH_POLL,
            watch_id,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub fn kbd_read_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_KBD_READ,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
            0,
        )
    }
}

pub fn graph_get_raw(request: &[u8], out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRAPH_GET,
            request.as_ptr() as u64,
            request.len() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

pub fn grant_capability_raw(payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_GRANT_CAPABILITY,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    }
}

fn serialize_request<T: Serialize>(req: &T) -> Option<Vec<u8>> {
    postcard::to_allocvec(req).ok()
}

pub fn irq_bind(request: IrqBindRequest) -> Option<u64> {
    let payload = serialize_request(&request)?;
    let handle = unsafe {
        syscall(
            SYSCALL_IRQ_BIND,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    };
    (handle != !0).then_some(handle)
}

pub fn irq_ack(handle: u64) -> bool {
    unsafe { syscall(SYSCALL_IRQ_ACK, handle, 0, 0, 0) == 0 }
}

pub fn dma_map(request: DmaMapRequest) -> Option<u64> {
    let payload = serialize_request(&request)?;
    let handle = unsafe {
        syscall(
            SYSCALL_DMA_MAP,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    };
    (handle != !0).then_some(handle)
}

pub fn dma_submit(request: DmaSubmitRequest) -> Option<u64> {
    let payload = serialize_request(&request)?;
    let handle = unsafe {
        syscall(
            SYSCALL_DMA_SUBMIT,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        )
    };
    (handle != !0).then_some(handle)
}

pub fn dma_wait(request: DmaWaitRequest) -> bool {
    let Some(payload) = serialize_request(&request) else {
        return false;
    };
    unsafe {
        syscall(
            SYSCALL_DMA_WAIT,
            payload.as_ptr() as u64,
            payload.len() as u64,
            0,
            0,
        ) == 0
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    crate::graph::log_args(args);
}
