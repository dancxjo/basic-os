//! Telemetry primitives: symbols, a proposition journal, and a graph of Things.
//! These are intentionally small and declarative so other subsystems (or an LLM)
//! can register kinds/predicates, log facts, and store opaque data safely.

pub mod canon;
pub mod graph;
pub mod journal;

use spin::Once;

static INIT: Once<()> = Once::new();

/// Initialize telemetry components. Idempotent.
pub fn init() {
    INIT.call_once(|| {
        journal::init();
        graph::init();
    });
}
