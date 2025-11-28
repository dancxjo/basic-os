use crate::graph::canon::Symbol;
use crate::graph::journal::Value;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifier representing a bundle/authority.
pub type BundleId = Uuid;

/// Stable identifier for the kernel bundle. This bundle implicitly holds
/// all privileges and is used for early boot declarations.
pub const KERNEL_BUNDLE_ID: BundleId = Uuid::from_u128(0xfeed_cafe_dead_beef_cafe_babe_0000_0001);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphChange {
    Thing(GraphThing),
    Edge(GraphEdge),
}

impl GraphChange {
    pub fn revision(&self) -> u64 {
        match self {
            GraphChange::Thing(t) => t.revision,
            GraphChange::Edge(e) => e.revision,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThing {
    pub id: Uuid,
    pub kind: Symbol,
    pub labels: BTreeSet<Symbol>,
    pub fields: BTreeMap<Symbol, Value>,
    pub owner: BundleId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: Uuid,
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub props: BTreeMap<Symbol, Value>,
    pub owner: BundleId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub revision: u64,
    pub thing_count: usize,
    pub edge_count: usize,
    pub things: Vec<GraphThing>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphWatchBatch {
    pub from_revision: u64,
    pub latest_revision: u64,
    pub changes: Vec<GraphChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFiatRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub fields: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThatRequest {
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub revision_hint: u64,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchQuery {
    pub kind: Option<Symbol>,
    pub src: Option<Uuid>,
    pub dst: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodePattern {
    pub labels: Vec<Symbol>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeRequest {
    pub id: Option<Uuid>,
    pub labels: Vec<Symbol>,
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub from: Uuid,
    pub to: Uuid,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsRequest {
    pub node: Uuid,
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsGetRequest {
    pub node: Uuid,
    pub keys: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedBufferSpec {
    pub id: Option<Uuid>,
    pub size_bytes: u64,
    pub kind: Symbol,
    pub usage: Symbol,
    #[serde(default)]
    pub addr: Option<u64>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStateSpec {
    pub id: Option<Uuid>,
    pub buffer: Uuid,
    pub owner: BundleId,
    pub head: u64,
    pub tail: u64,
    pub has_data: bool,
    #[serde(default)]
    pub capacity: Option<u64>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

/// Request to grant a capability from one bundle to another.
/// Data capabilities (CAN_READ/CAN_WRITE/CAN_LINK) may be delegated by the
/// owner of the target Thing. Hardware capabilities (IRQ, DMA, MMIO, PORT IO)
/// are kernel-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityRequest {
    /// The bundle receiving the capability
    pub grantee: BundleId,
    /// The target node the capability applies to
    pub target: Uuid,
    /// The capability being granted (e.g., CAN_READ, CAN_WRITE, CAN_LINK)
    pub capability: Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindByKind {
    pub kind_ptr: u64, // *const u8
    pub kind_len: u64, // usize
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindResultHeader {
    pub next_cursor: u64,
    pub count: u32,
}

pub type WatchId = u64;

pub struct Watch {
    pub(crate) id: WatchId,
    pub(crate) owner: BundleId,
    pub(crate) pattern: NodePattern,
    pub(crate) queue: Vec<GraphChange>,
}
