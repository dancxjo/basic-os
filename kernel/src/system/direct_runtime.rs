use crate::graph::{self, NodePattern};
use crate::task::runtime::current_bundle;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use thing_abi::{
    AbiRequest, AbiResponse, FramebufferGeometry, GrantCapabilityRequest, GraphFiatRequest,
    GraphLinkRequest, GraphPropsRequest, GraphThatRequest, GraphThing, GraphWatchBatch,
    ThingRuntime, WatchQuery,
};
use userland::sys::{DmaMapRequest, DmaSubmitRequest, DmaWaitRequest, IrqBindRequest};

pub struct DirectKernelRuntime;

impl ThingRuntime for DirectKernelRuntime {
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
                let bundle = current_bundle();
                let thing = graph::fiat_for_bundle(bundle, request);
                AbiResponse::Fiat { thing }
            }
            AbiRequest::Link {
                id,
                from,
                pred,
                to,
                props,
            } => {
                let request = GraphLinkRequest {
                    id,
                    from,
                    to,
                    pred: pred.clone(),
                    props: props.clone(),
                };
                let bundle = current_bundle();
                let revision = graph::link(bundle, request);

                let edge = thing_abi::GraphEdge {
                    id: id.unwrap_or_else(thing_abi::next_uuid),
                    src: from,
                    pred: pred,
                    dst: to,
                    props: props,
                    owner: uuid::Uuid::nil(), // TODO: get owner
                    revision,
                };
                AbiResponse::Link { edge: Some(edge) }
            }
            AbiRequest::Get { id } => {
                let thing = graph::get_thing(&id);
                AbiResponse::Get { thing }
            }
            AbiRequest::Query { pattern } => AbiResponse::Error {
                message: "Query not implemented".into(),
            },
            AbiRequest::FindByKind { kind, cursor } => {
                let bundle = current_bundle();
                let (things, next_cursor) = graph::find_by_kind(bundle, &kind, cursor.unwrap_or(0));
                AbiResponse::Find {
                    things,
                    next_cursor: Some(next_cursor),
                }
            }
            AbiRequest::WatchRegister { pattern } => {
                let bundle = current_bundle();
                let handle = graph::register_watch_pattern(bundle, pattern);
                AbiResponse::WatchRegistered { watch_id: handle }
            }
            AbiRequest::WatchPoll {
                watch_id,
                max_events: _,
            } => {
                let batch = graph::poll_watch(watch_id).unwrap_or_else(|| GraphWatchBatch {
                    from_revision: 0,
                    latest_revision: 0,
                    changes: Vec::new(),
                });
                AbiResponse::WatchEvents { events: batch }
            }
            AbiRequest::DevOpen { kind, index: _ } => {
                // 1=Keyboard, 2=Mouse (from userland::sys)
                let handle = match kind {
                    1 => 1,
                    2 => 2,
                    _ => 0,
                };
                AbiResponse::DevOpened {
                    handle: if handle > 0 { Some(handle) } else { None },
                }
            }
            AbiRequest::DevRead { handle, len } => {
                let mut buf = vec![0u8; len];
                let n = match handle {
                    1 => crate::drivers::keyboard::read_keyboard(&mut buf),
                    2 => crate::drivers::mouse::read_mouse(&mut buf),
                    _ => 0,
                };
                buf.truncate(n);
                AbiResponse::DevRead { data: buf }
            }
            AbiRequest::IrqBind { device: _, line: _ } => AbiResponse::IrqBound { handle: Some(1) },
            AbiRequest::IrqAck { handle: _ } => AbiResponse::IrqAcked,
            AbiRequest::FbInfo => {
                if let Some(info) = crate::drivers::framebuffer::get_framebuffer_info() {
                    AbiResponse::FbInfo {
                        info: FramebufferGeometry {
                            width: info.width as u32,
                            height: info.height as u32,
                            pitch: info.pitch as u32,
                            bpp: info.bpp as u16,
                        },
                    }
                } else {
                    AbiResponse::Error {
                        message: "No framebuffer".into(),
                    }
                }
            }
            AbiRequest::FbMap => {
                if let Some(info) = crate::drivers::framebuffer::get_framebuffer_info() {
                    AbiResponse::FbMapped { addr: info.addr }
                } else {
                    AbiResponse::Error {
                        message: "No framebuffer".into(),
                    }
                }
            }
            _ => AbiResponse::Error {
                message: "Not implemented".into(),
            },
        }
    }
}
