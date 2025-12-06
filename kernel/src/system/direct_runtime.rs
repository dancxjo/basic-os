use crate::graph::api;
use crate::graph::types::KERNEL_BUNDLE_ID;
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use thing_abi::{
    AbiRequest, AbiResponse, GrantCapabilityRequest, GraphEdge, GraphFiatRequest,
    GraphPropsRequest, GraphThatRequest, ThingRuntime,
};
use uuid::Uuid;

pub struct KernelDirectRuntime;

impl ThingRuntime for KernelDirectRuntime {
    fn call(&self, req: AbiRequest) -> AbiResponse {
        match req {
            AbiRequest::Fiat {
                id,
                kind,
                labels,
                fields,
            } => {
                let request = GraphFiatRequest {
                    id,
                    kind,
                    labels,
                    fields,
                };
                let thing = api::fiat(request);
                AbiResponse::Fiat { thing }
            }
            AbiRequest::Link {
                id,
                from,
                pred,
                to,
                props,
            } => {
                let request = GraphThatRequest {
                    src: from,
                    pred: pred.clone(),
                    dst: to,
                    revision_hint: 0,
                    props: props.clone(),
                };
                let revision = api::that(request);
                let edge_id = id.unwrap_or_else(|| thing_abi::next_uuid());
                let edge = GraphEdge {
                    id: edge_id,
                    src: from,
                    pred,
                    dst: to,
                    props,
                    owner: Uuid::nil(),
                    revision,
                };
                AbiResponse::Link { edge: Some(edge) }
            }
            AbiRequest::Get { id } => {
                let thing =
                    api::with_store(|store: &mut crate::graph::store::Store| store.latest(&id));
                AbiResponse::Get { thing }
            }
            AbiRequest::Query { pattern } => {
                let things = api::with_store(|store: &mut crate::graph::store::Store| {
                    store.get_nodes(KERNEL_BUNDLE_ID, pattern)
                });
                AbiResponse::Query { things }
            }
            AbiRequest::FindByKind { kind, cursor } => {
                let (things, next_cursor) =
                    api::find_by_kind(KERNEL_BUNDLE_ID, &kind, cursor.unwrap_or(0));
                AbiResponse::Find {
                    things,
                    next_cursor: Some(next_cursor),
                }
            }
            AbiRequest::WatchRegister { pattern } => {
                let watch_id = api::register_watch_pattern(KERNEL_BUNDLE_ID, pattern);
                AbiResponse::WatchRegistered { watch_id }
            }
            AbiRequest::WatchPoll {
                watch_id,
                max_events: _,
            } => {
                if let Some(batch) = api::poll_watch(watch_id) {
                    AbiResponse::WatchEvents { events: batch }
                } else {
                    AbiResponse::WatchEvents {
                        events: thing_abi::GraphWatchBatch {
                            changes: Vec::new(),
                            latest_revision: 0,
                            from_revision: 0,
                        },
                    }
                }
            }
            AbiRequest::GrantCapability { request } => {
                let success = api::grant_capability(KERNEL_BUNDLE_ID, request);
                AbiResponse::CapabilityGranted { granted: success }
            }
            _ => AbiResponse::Error {
                message: "Not implemented in KernelDirectRuntime".into(),
            },
        }
    }
}

pub unsafe fn syscall_handler(rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64) -> u64 {
    match rax {
        1 => {
            // SYS_LOG
            let ptr = rdi as *const u8;
            let len = rsi as usize;
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(s) = core::str::from_utf8(slice) {
                log::info!("{}", s.trim());
            }
            0
        }
        0x09 => {
            // SYSCALL_FB_INFO
            // rdi: out_ptr, rsi: out_len
            let out_ptr = rdi;
            let out_len = rsi;
            if out_ptr == 0 || out_len == 0 {
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
            let required = bytes.len() as u64;
            if out_len < required {
                return required;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr as *mut u8, bytes.len());
            }
            required
        }
        0x0A => {
            // SYSCALL_FB_MAP
            match crate::drivers::framebuffer::get_framebuffer_info() {
                Some(info) => info.addr,
                None => 0,
            }
        }
        0x20 => {
            // SYSCALL_DEV_OPEN
            // rdi: kind, rsi: index
            if let Some(handle) = crate::drivers::device::dev_open(rdi as u32, rsi as usize) {
                handle
            } else {
                0
            }
        }
        0x21 => {
            // SYSCALL_DEV_READ
            // rdi: handle, rsi: out_ptr, rdx: out_len
            let handle = rdi;
            let out_ptr = rsi;
            let out_len = rdx;
            if out_ptr == 0 || out_len == 0 {
                return 0;
            }
            let buf =
                unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, out_len as usize) };
            crate::drivers::device::dev_read(handle, buf) as u64
        }
        0x11 => {
            // SYSCALL_IRQ_BIND
            // rdi: ptr to IrqBindRequest, rsi: len
            // We need to deserialize the request.
            // But wait, syscalls usually pass structs by pointer.
            // kernel/src/arch/x86_64/syscall.rs uses `irq_bind(rdi, rsi)`.
            // Let's check `irq_bind` implementation in syscall.rs.
            // It deserializes using postcard.

            let ptr = rdi as *const u8;
            let len = rsi as usize;
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(req) =
                postcard::from_bytes::<crate::arch::x86_64::syscall::IrqBindRequest>(slice)
            {
                if let Some(handle) =
                    crate::drivers::irq_dma::irq_bind(KERNEL_BUNDLE_ID, req.device, req.irq_line)
                {
                    handle
                } else {
                    0
                }
            } else {
                0
            }
        }
        0x30 => {
            // SYSCALL_SPAWN
            // rdi: ptr, rsi: len
            let ptr = rdi as *const u8;
            let len = rsi as usize;
            if ptr.is_null() || len == 0 || len > 128 {
                return !0;
            }
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(name) = core::str::from_utf8(slice) {
                #[cfg(feature = "standalone_compositor")]
                {
                    compositor::request_spawn(name);
                    0
                }
                #[cfg(not(feature = "standalone_compositor"))]
                {
                    log::warn!("Spawn not supported without standalone_compositor");
                    !0
                }
            } else {
                !0
            }
        }
        _ => {
            log::warn!("Unhandled syscall in standalone mode: {}", rax);
            0
        }
    }
}
