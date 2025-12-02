use alloc::vec::Vec;
#[allow(unused_imports)] // Re-export graph ABI types for downstream consumers.
pub use thing_abi::{
    GrantCapabilityRequest, GraphChange, GraphEdge, GraphFiatRequest, GraphFindByKind,
    GraphFindResultHeader, GraphGetRequest, GraphLinkRequest, GraphNodeRequest,
    GraphPropsGetRequest, GraphPropsRequest, GraphSnapshot, GraphThatRequest, GraphThing,
    GraphWatchBatch, NodePattern, QueueStateSpec, SharedBufferSpec, Symbol, Value, WatchId, canon,
    cc, from_char, from_u16,
};

#[allow(unused_imports)] // Re-export ID helpers even if unused in this crate.
pub use thingos_kernel_std::id::{BundleId, KERNEL_BUNDLE_ID, PredId, ThingId};

#[allow(unused_imports)] // Provide alternative symbol helpers.
pub use thing_abi::{
    canon as canon_sym, cc as cc_sym, from_char as symbol_from_char, from_u16 as symbol_from_u16,
};

pub struct Watch {
    pub(crate) id: WatchId,
    pub(crate) owner: BundleId,
    pub(crate) pattern: NodePattern,
    pub(crate) queue: Vec<GraphChange>,
}
