use crate::{
    serial_println,
    thing::{Space, Uri},
};
use alloc::{collections::BTreeMap, vec::Vec};
use alloc::{string::String, vec};
use x86_64::VirtAddr;

/// A diagnostic overlay that prints memory and fact-based graph info to the serial console.
pub struct DebugOverlay<'a> {
    graph: &'a dyn Space,
    heap_start: VirtAddr,
    heap_size: usize,
}

impl<'a> DebugOverlay<'a> {
    pub fn new(graph: &'a dyn Space, heap_start: VirtAddr, heap_size: usize) -> Self {
        DebugOverlay {
            graph,
            heap_start,
            heap_size,
        }
    }

    pub fn print(&self) {
        serial_println!("\n--- ThingOS Debug Overlay ---");
        serial_println!(
            "Heap range: {:#x} - {:#x}",
            self.heap_start.as_u64(),
            self.heap_start.as_u64() + self.heap_size as u64
        );

        let mut thing_count = 0;
        let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();

        // Naive implementation: we iterate over known kernel Things
        for uri in self.enumerate_known_uris() {
            thing_count += 1;
            if let Some(kind) = self.graph.kind(&uri) {
                let kind_key = kind.0.clone(); // This is a String
                *kind_counts.entry(kind_key).or_insert(0) += 1;
            }
        }

        serial_println!("Number of Things: {}", thing_count);
        for (kind, count) in kind_counts {
            serial_println!("  [{}]: {}", kind, count);
        }
    }

    /// Enumerate a fixed list of known URIs for now. You can expand this or introspect later.
    fn enumerate_known_uris(&self) -> Vec<Uri> {
        vec![
            Uri("os://kernel".into()),
            Uri("ui://screen/1".into()),
            Uri("proc://1".into()),
        ]
    }
}

#[macro_export]
macro_rules! dump_overlay {
    ($graph:expr) => {{
        use $crate::overlay::DebugOverlay;
        let overlay = DebugOverlay::new(
            $graph,
            VirtAddr::new(crate::memory::HEAP_START),
            crate::memory::HEAP_SIZE,
        );
        overlay.print();
    }};
}
