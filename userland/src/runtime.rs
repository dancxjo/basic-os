use alloc::vec;
use alloc::vec::Vec;
use core::cmp::min;

use thing_abi::{
    runtime as abi_runtime, set_runtime as abi_set_runtime, AbiRequest, AbiResponse, GraphEdge,
    GraphFiatRequest, GraphGetRequest, GraphPropsGetRequest, GraphPropsRequest, GraphThatRequest,
    GraphThing, GraphWatchBatch, Map, NodePattern, ThingRuntime,
};
use uuid::Uuid;

use crate::sys;

pub struct KernelRuntime;

impl KernelRuntime {
    pub const fn new() -> Self {
        Self
    }
}

impl ThingRuntime for KernelRuntime {
    fn call(&self, req: AbiRequest) -> AbiResponse {
        match req {
            AbiRequest::Fiat {
                id,
                kind,
                mut labels,
                fields,
            } => {
                if labels.is_empty() {
                    labels.push(kind);
                }
                let request = GraphFiatRequest {
                    id,
                    kind,
                    labels,
                    fields,
                };
                let Ok(buf) = postcard::to_allocvec(&request) else {
                    return AbiResponse::Error {
                        message: "serialize fiat".into(),
                    };
                };
                let revision = sys::graph_fiat_raw(&buf);
                let thing_id = request.id.unwrap_or_else(|| thing_abi::next_uuid());
                let thing = match fetch_thing(thing_id) {
                    Some(mut thing) => {
                        thing.revision = revision;
                        thing
                    }
                    None => GraphThing {
                        id: thing_id,
                        kind: request.kind,
                        labels: request.labels.iter().copied().collect(),
                        fields: request.fields,
                        owner: Uuid::nil(),
                        revision,
                    },
                };
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
                    props,
                };
                if let Ok(buf) = postcard::to_allocvec(&request) {
                    let revision = sys::graph_link_raw(&buf);
                    let edge_id = id.unwrap_or_else(|| thing_abi::next_uuid());
                    let edge = GraphEdge {
                        id: edge_id,
                        src: from,
                        pred,
                        dst: to,
                        props: request.props,
                        owner: Uuid::nil(),
                        revision,
                    };
                    return AbiResponse::Link { edge: Some(edge) };
                }
                AbiResponse::Error {
                    message: "link serialize".into(),
                }
            }
            AbiRequest::Get { id } => AbiResponse::Get {
                thing: fetch_thing(id),
            },
            AbiRequest::Query { pattern } => AbiResponse::Query {
                things: fetch_things(pattern),
            },
            AbiRequest::FindByKind { kind, cursor } => {
                let mut results = Vec::new();
                let mut next = cursor.unwrap_or(0);
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let req = thing_abi::GraphFindByKind {
                        kind_ptr: kind.as_ptr() as u64,
                        kind_len: kind.len() as u64,
                        cursor: next,
                    };
                    let written = sys::graph_find_by_kind_raw(&req, &mut buf);
                    if written == !0 {
                        break;
                    }
                    let slice = &buf[..min(written as usize, buf.len())];
                    if let Ok((header, rest)) =
                        postcard::take_from_bytes::<thing_abi::GraphFindResultHeader>(slice)
                    {
                        let mut remaining = rest;
                        for _ in 0..header.count {
                            if let Ok((thing, next_slice)) =
                                postcard::take_from_bytes::<GraphThing>(remaining)
                            {
                                results.push(thing);
                                remaining = next_slice;
                            }
                        }
                        if header.next_cursor == 0 {
                            next = 0;
                            break;
                        }
                        next = header.next_cursor;
                    } else {
                        break;
                    }
                }
                AbiResponse::Find {
                    things: results,
                    next_cursor: (next != 0).then_some(next),
                }
            }
            AbiRequest::WatchRegister { pattern } => match postcard::to_allocvec(&pattern) {
                Ok(buf) => {
                    let id = sys::graph_watch_register_raw(&buf);
                    if id == !0 {
                        AbiResponse::Error {
                            message: "watch register failed".into(),
                        }
                    } else {
                        AbiResponse::WatchRegistered { watch_id: id }
                    }
                }
                Err(_) => AbiResponse::Error {
                    message: "watch serialize".into(),
                },
            },
            AbiRequest::WatchUnregister { watch_id: _ } => AbiResponse::WatchUnregistered,
            AbiRequest::WatchPoll { watch_id, .. } => {
                let mut buf = vec![0u8; 64 * 1024];
                let len = sys::graph_watch_poll_raw(watch_id, &mut buf);
                if len == !0 {
                    return AbiResponse::WatchEvents {
                        events: GraphWatchBatch {
                            from_revision: 0,
                            latest_revision: 0,
                            changes: Vec::new(),
                        },
                    };
                }
                let batch = postcard::from_bytes::<GraphWatchBatch>(&buf[..len as usize])
                    .unwrap_or(GraphWatchBatch {
                        from_revision: 0,
                        latest_revision: 0,
                        changes: Vec::new(),
                    });
                AbiResponse::WatchEvents { events: batch }
            }
            AbiRequest::PropsGet { request } => match postcard::to_allocvec(&request) {
                Ok(encoded) => {
                    let mut buf = vec![0u8; 4096];
                    let len = sys::graph_get_props_raw(&encoded, &mut buf);
                    if len == !0 {
                        AbiResponse::Error {
                            message: "props get failed".into(),
                        }
                    } else {
                        let props =
                            postcard::from_bytes::<Map>(&buf[..len as usize]).unwrap_or_default();
                        AbiResponse::Props { props }
                    }
                }
                Err(_) => AbiResponse::Error {
                    message: "props serialize".into(),
                },
            },
            AbiRequest::PropsSet { request } => match postcard::to_allocvec(&request) {
                Ok(buf) => {
                    let ok = sys::graph_set_props_raw(&buf) == 0;
                    if ok {
                        AbiResponse::Props {
                            props: request.props,
                        }
                    } else {
                        AbiResponse::Error {
                            message: "props set failed".into(),
                        }
                    }
                }
                Err(_) => AbiResponse::Error {
                    message: "props serialize".into(),
                },
            },
            AbiRequest::GrantCapability { request } => match postcard::to_allocvec(&request) {
                Ok(buf) => AbiResponse::CapabilityGranted {
                    granted: sys::grant_capability_raw(&buf) == 0,
                },
                Err(_) => AbiResponse::CapabilityGranted { granted: false },
            },
        }
    }
}

fn fetch_things(pattern: NodePattern) -> Vec<GraphThing> {
    let mut buf = vec![0u8; 64 * 1024];
    let Ok(encoded) = postcard::to_allocvec(&GraphGetRequest::Pattern(pattern)) else {
        return Vec::new();
    };
    let len = sys::graph_get_raw(&encoded, &mut buf);
    if len == !0 {
        return Vec::new();
    }
    postcard::from_bytes::<Vec<GraphThing>>(&buf[..len as usize]).unwrap_or_default()
}

fn fetch_thing(id: Uuid) -> Option<GraphThing> {
    let mut buf = vec![0u8; 4096];
    let Ok(encoded) = postcard::to_allocvec(&GraphGetRequest::Thing(id)) else {
        return None;
    };
    let len = sys::graph_get_raw(&encoded, &mut buf);
    if len == !0 {
        return None;
    }
    postcard::from_bytes::<GraphThing>(&buf[..len as usize]).ok()
}

static KERNEL_RUNTIME: KernelRuntime = KernelRuntime;

pub fn ensure_kernel_runtime() {
    let _ = abi_set_runtime(&KERNEL_RUNTIME);
}

pub fn runtime() -> &'static dyn ThingRuntime {
    abi_runtime()
}

pub fn set_runtime(runtime: &'static dyn ThingRuntime) {
    let _ = abi_set_runtime(runtime);
}
