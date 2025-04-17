use crate::serial_println;
use crate::thing::Graph;
use x86_64::VirtAddr;

/// A diagnostic overlay that prints basic memory and graph info to the serial console.
pub struct DebugOverlay<'a> {
    graph: &'a Graph,
    heap_start: VirtAddr,
    heap_size: usize,
}

impl<'a> DebugOverlay<'a> {
    pub fn new(graph: &'a Graph, heap_start: VirtAddr, heap_size: usize) -> Self {
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
        serial_println!("Number of Things: {}", self.graph.things.len());

        let kinds = self.count_kinds();
        for (kind, count) in kinds {
            serial_println!("  [{}]: {}", kind, count);
        }
    }

    fn count_kinds(&self) -> alloc::collections::BTreeMap<&'static str, usize> {
        let mut counts = alloc::collections::BTreeMap::new();
        for thing in &self.graph.things {
            *counts.entry(thing.kind).or_insert(0) += 1;
        }
        counts
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
