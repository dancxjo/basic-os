use alloc::vec::Vec;
pub use thing_abi::{
    BundleId, GrantCapabilityRequest, GraphChange, GraphEdge, GraphFiatRequest, GraphFindByKind,
    GraphFindResultHeader, GraphGetRequest, GraphLinkRequest, GraphNodeRequest,
    GraphPropsGetRequest, GraphPropsRequest, GraphSnapshot, GraphThatRequest, GraphThing,
    GraphWatchBatch, KERNEL_BUNDLE_ID, NodePattern, QueueStateSpec, SharedBufferSpec, Symbol,
    Value, WatchId, canon, cc, from_char, from_u16,
};

pub use thing_abi::{
    canon as canon_sym, cc as cc_sym, from_char as symbol_from_char, from_u16 as symbol_from_u16,
};

pub struct Watch {
    pub(crate) id: WatchId,
    pub(crate) owner: BundleId,
    pub(crate) pattern: NodePattern,
    pub(crate) queue: Vec<GraphChange>,
}
