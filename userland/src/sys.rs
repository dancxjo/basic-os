use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};
use thing_abi::GraphFindByKind;
use uuid::Uuid;

/// Raw syscall entry point (rax, rdi, rsi, rdx, r10).
#[inline(always)]
#[cfg(target_os = "none")]
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

#[cfg(not(target_os = "none"))]
pub unsafe fn syscall(_rax: u64, _rdi: u64, _rsi: u64, _rdx: u64, _r10: u64) -> u64 {
    panic!("syscall not supported on host");
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
pub const SYSCALL_DEV_OPEN: u64 = 0x20;
pub const SYSCALL_DEV_READ: u64 = 0x21;
pub const SYSCALL_DEV_WRITE: u64 = 0x22;
pub const SYSCALL_DEV_MAP: u64 = 0x23;
pub const SYSCALL_SPAWN: u64 = 0x30;
pub const SYSCALL_LOG: u64 = 0x99;

pub fn spawn(name: &str) -> u64 {
    #[cfg(target_os = "none")]
    {
        unsafe { syscall(SYSCALL_SPAWN, name.as_ptr() as u64, name.len() as u64, 0, 0) }
    }
    #[cfg(not(target_os = "none"))]
    {
        #[cfg(feature = "std")]
        return spawn_host(name);
        #[cfg(not(feature = "std"))]
        return 0;
    }
}

#[cfg(feature = "std")]
static HOST_APPS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, fn() -> !>>> = std::sync::OnceLock::new();

#[cfg(feature = "std")]
pub fn register_host_app(name: &str, func: fn() -> !) {
    let apps = HOST_APPS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    apps.lock().unwrap().insert(name.to_string(), func);
}

#[cfg(feature = "std")]
fn spawn_host(name: &str) -> u64 {
    if let Some(apps) = HOST_APPS.get() {
        if let Some(func) = apps.lock().unwrap().get(name) {
            let func = *func;
            std::thread::spawn(move || {
                func();
            });
            return 1;
        }
    }
    println!("Host spawn failed: app '{}' not registered", name);
    0
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
    #[cfg(target_os = "none")]
    {
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
        if ret == core::mem::size_of::<FramebufferInfo>() as u64 {
            Some(info)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::FbInfo;
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::FbInfo { info } => Some(FramebufferInfo {
                width: info.width as u64,
                height: info.height as u64,
                pitch: info.pitch as u64,
                bpp: info.bpp as u64,
                addr: 0, // Addr is not returned by info, but by map
            }),
            _ => None,
        }
    }
}

pub fn fb_map() -> u64 {
    #[cfg(target_os = "none")]
    {
        unsafe { syscall(SYSCALL_FB_MAP, 0, 0, 0, 0) }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::FbMap;
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::FbMapped { addr } => addr,
            _ => 0,
        }
    }
}

pub fn graph_fiat_raw(payload: &[u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GraphFiatRequest = postcard::from_bytes(payload).expect("deserialize fiat");
        let abi_req = thing_abi::AbiRequest::Fiat {
            id: req.id,
            kind: req.kind,
            labels: req.labels,
            fields: req.fields,
        };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Fiat { thing } => thing.revision,
            _ => 0,
        }
    }
}

pub fn graph_find_by_kind_raw(req: &GraphFindByKind, out: &mut [u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let kind_slice = unsafe { core::slice::from_raw_parts(req.kind_ptr as *const u8, req.kind_len as usize) };
        let kind_str = core::str::from_utf8(kind_slice).unwrap_or("");
        let abi_req = thing_abi::AbiRequest::FindByKind {
            kind: kind_str.to_string(),
            cursor: None,
        };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Find { things, .. } => {
                let slice = postcard::to_slice(&things, out).unwrap_or(&mut []);
                slice.len() as u64
            }
            _ => 0,
        }
    }
}

pub fn graph_link_raw(payload: &[u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GraphLinkRequest = postcard::from_bytes(payload).expect("deserialize link");
        let abi_req = thing_abi::AbiRequest::Link {
            id: req.id,
            from: req.from,
            pred: req.pred,
            to: req.to,
            props: req.props,
        };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Link { edge: Some(edge) } => edge.revision,
            _ => 0,
        }
    }
}

pub fn graph_get_props_raw(request: &[u8], out: &mut [u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GraphPropsGetRequest = postcard::from_bytes(request).expect("deserialize props get");
        let abi_req = thing_abi::AbiRequest::PropsGet { request: req };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Props { props } => {
                let slice = postcard::to_slice(&props, out).unwrap_or(&mut []);
                slice.len() as u64
            }
            _ => 0,
        }
    }
}

pub fn graph_set_props_raw(request: &[u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GraphPropsRequest = postcard::from_bytes(request).expect("deserialize props set");
        let abi_req = thing_abi::AbiRequest::PropsSet { request: req };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Props { .. } => 1, // Success
            _ => 0,
        }
    }
}

pub fn graph_watch_register_raw(payload: &[u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let pattern: thing_abi::NodePattern = postcard::from_bytes(payload).expect("deserialize watch pattern");
        let abi_req = thing_abi::AbiRequest::WatchRegister { pattern };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::WatchRegistered { watch_id } => watch_id,
            _ => 0,
        }
    }
}

pub fn graph_watch_poll_raw(watch_id: u64, out: &mut [u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let abi_req = thing_abi::AbiRequest::WatchPoll {
            watch_id,
            max_events: None,
        };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::WatchEvents { events } => {
                let slice = postcard::to_slice(&events, out).unwrap_or(&mut []);
                slice.len() as u64
            }
            _ => 0,
        }
    }
}

pub fn kbd_read_raw(out: &mut [u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        // Not implemented on host via this syscall, use DevRead or similar if needed
        0
    }
}

pub fn graph_get_raw(request: &[u8], out: &mut [u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GraphGetRequest = postcard::from_bytes(request).unwrap();
        let abi_req = match req {
            thing_abi::GraphGetRequest::Thing(id) => thing_abi::AbiRequest::Get { id },
            thing_abi::GraphGetRequest::Pattern(pattern) => thing_abi::AbiRequest::Query { pattern },
        };

        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::Get { thing: Some(node) } => {
                let slice = postcard::to_slice(&node, out).unwrap_or(&mut []);
                slice.len() as u64
            }
            thing_abi::AbiResponse::Query { things } => {
                let slice = postcard::to_slice(&things, out).unwrap_or(&mut []);
                slice.len() as u64
            }
            _ => 0,
        }
    }
}

pub fn grant_capability_raw(payload: &[u8]) -> u64 {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req: thing_abi::GrantCapabilityRequest = postcard::from_bytes(payload).expect("deserialize grant cap");
        let abi_req = thing_abi::AbiRequest::GrantCapability { request: req };
        match crate::runtime().call(abi_req) {
            thing_abi::AbiResponse::CapabilityGranted { granted } => if granted { 0 } else { 1 },
            _ => 1,
        }
    }
}

fn serialize_request<T: Serialize>(req: &T) -> Option<Vec<u8>> {
    postcard::to_allocvec(req).ok()
}

pub fn irq_bind(request: IrqBindRequest) -> Option<u64> {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::IrqBind {
            device: request.device,
            line: request.irq_line,
        };
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::IrqBound { handle } => handle,
            _ => None,
        }
    }
}

pub fn irq_ack(handle: u64) -> bool {
    #[cfg(target_os = "none")]
    {
        unsafe { syscall(SYSCALL_IRQ_ACK, handle, 0, 0, 0) == 0 }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::IrqAck { handle };
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::IrqAcked => true,
            _ => false,
        }
    }
}

pub fn dma_map(request: DmaMapRequest) -> Option<u64> {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        None
    }
}

pub fn dma_submit(request: DmaSubmitRequest) -> Option<u64> {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        None
    }
}

pub fn dma_wait(request: DmaWaitRequest) -> bool {
    #[cfg(target_os = "none")]
    {
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
    #[cfg(not(target_os = "none"))]
    {
        false
    }
}

pub fn log(s: &str) {
    #[cfg(target_os = "none")]
    {
        unsafe { syscall(SYSCALL_LOG, s.as_ptr() as u64, s.len() as u64, 0, 0) };
    }
    #[cfg(not(target_os = "none"))]
    {
        // On host, print to stderr
        eprint!("{}", s);
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    crate::graph::log_args(args);
}

pub const DEVICE_KIND_KEYBOARD: u32 = 1;
pub const DEVICE_KIND_MOUSE: u32 = 2;
pub const DEVICE_KIND_FRAMEBUFFER: u32 = 3;
pub const DEVICE_KIND_SERIAL: u32 = 4;

pub fn dev_open(kind: u32, index: usize) -> Option<u64> {
    #[cfg(target_os = "none")]
    {
        let ret = unsafe { syscall(SYSCALL_DEV_OPEN, kind as u64, index as u64, 0, 0) };
        if ret == !0 {
            None
        } else {
            Some(ret)
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::DevOpen { kind, index };
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::DevOpened { handle } => handle,
            _ => None,
        }
    }
}

pub fn dev_read(handle: u64, buf: &mut [u8]) -> usize {
    #[cfg(target_os = "none")]
    {
        unsafe {
            syscall(
                SYSCALL_DEV_READ,
                handle,
                buf.as_mut_ptr() as u64,
                buf.len() as u64,
                0,
            ) as usize
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::DevRead {
            handle,
            len: buf.len(),
        };
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::DevRead { data } => {
                let len = core::cmp::min(buf.len(), data.len());
                buf[..len].copy_from_slice(&data[..len]);
                len
            }
            _ => 0,
        }
    }
}

pub fn dev_write(handle: u64, buf: &[u8]) -> usize {
    #[cfg(target_os = "none")]
    {
        unsafe {
            syscall(
                SYSCALL_DEV_WRITE,
                handle,
                buf.as_ptr() as u64,
                buf.len() as u64,
                0,
            ) as usize
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        let req = thing_abi::AbiRequest::DevWrite {
            handle,
            data: buf.to_vec(),
        };
        match crate::runtime().call(req) {
            thing_abi::AbiResponse::DevWritten { len } => len,
            _ => 0,
        }
    }
}

pub fn dev_map(handle: u64) -> Option<u64> {
    #[cfg(target_os = "none")]
    {
        let ret = unsafe { syscall(SYSCALL_DEV_MAP, handle, 0, 0, 0) };
        if ret == 0 {
            None
        } else {
            Some(ret)
        }
    }
    #[cfg(not(target_os = "none"))]
    {
        None
    }
}
