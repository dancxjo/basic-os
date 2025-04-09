use crate::graph::{Graph, ThingData};
use alloc::{boxed::Box, format};

use limine::memory_map::EntryType;
use limine::request::MemoryMapRequest;

#[used]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

unsafe extern "C" {
    static _start: u8;
    static _etext: u8;
    static _edata: u8;
    static _end: u8;
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RegionView {
    base: usize,
    length: usize,
}

pub fn thingify_memory_region(
    graph: &mut Graph,
    name: &'static str,
    kind: &'static str,
    start: usize,
    length: usize,
) -> usize {
    graph.add_kind(kind, ""); // idempotent
    let view = RegionView {
        base: start,
        length,
    };
    graph.create_typed(name, kind, view)
}

pub fn thingify_kernel(graph: &mut Graph) {
    graph.add_kind("process", "A running unit of code");
    graph.add_kind("segment", "Code or data segment");
    graph.add_predicate("contains", "process", "segment");

    let kernel_id = graph.create_typed("kernel", "process", ());

    let mut seg = |name, start: *const u8, end: *const u8| {
        let len = end as usize - start as usize;
        let seg_id = thingify_memory_region(graph, name, "segment", start as usize, len);
        graph.link(kernel_id, seg_id, "contains");
    };

    unsafe {
        seg("kernel.text", &_start, &_etext);
        seg("kernel.data", &_etext, &_edata);
        seg("kernel.bss", &_edata, &_end);
    }
}

#[derive(Clone, Copy)]
pub struct MemoryRegion {
    pub base: u64,
    pub len: u64,
    pub kind: &'static str,
}

pub fn collect_memory_regions() -> &'static [MemoryRegion] {
    let response = MEMMAP_REQUEST.get_response().expect("No memory map");
    let entries = response.entries();

    static mut MEMORY_REGIONS: [MemoryRegion; 128] = [MemoryRegion {
        base: 0,
        len: 0,
        kind: "unknown",
    }; 128];

    let mut count = 0;

    for entry in entries {
        let kind = match entry.entry_type {
            EntryType::USABLE => "usable",
            EntryType::RESERVED => "reserved",
            EntryType::ACPI_RECLAIMABLE => "acpi_reclaimable",
            EntryType::ACPI_NVS => "acpi_nvs",
            EntryType::BAD_MEMORY => "bad_memory",
            EntryType::BOOTLOADER_RECLAIMABLE => "bootloader_reclaimable",
            EntryType::KERNEL_AND_MODULES => "kernel_and_modules",
            EntryType::FRAMEBUFFER => "framebuffer",
            _ => "unknown",
        };

        unsafe {
            MEMORY_REGIONS[count] = MemoryRegion {
                base: entry.base,
                len: entry.length,
                kind,
            };
            count += 1;
        }
    }

    unsafe { &MEMORY_REGIONS[..count] }
}

pub fn thingify_boot_memory(graph: &mut Graph, regions: &[MemoryRegion]) {
    graph.add_kind("boot_map", "The boot-time memory map");
    graph.add_kind("region", "A region of memory");
    graph.add_predicate("contains", "boot_map", "region");

    let map_id = graph.create_typed("boot.map", "boot_map", ());

    for (i, region) in regions.iter().enumerate() {
        let name = match region.kind {
            "usable" => format!("region.{}.usable", i),
            "reserved" => format!("region.{}.reserved", i),
            _ => format!("region.{}.{}", i, region.kind),
        };

        let boxed = Box::leak(name.into_boxed_str());
        let view = RegionView {
            base: region.base as usize,
            length: region.len as usize,
        };

        let region_id = graph.create_typed(boxed, "region", view);
        graph.link(map_id, region_id, "contains");
    }
}

pub fn thingify_ui_layout(graph: &mut Graph) {
    graph.add_kind("view-root", "Top-level UI view");
    graph.add_kind("background", "UI background");
    graph.add_kind("window", "Window container");
    graph.add_kind("pointer", "UI cursor/pointer");
    graph.add_kind("label", "Static UI text");
    graph.add_kind("log-view", "Scrollable log window");

    graph.add_predicate("contains", "view-root", "background");
    graph.add_predicate("contains", "view-root", "window");
    graph.add_predicate("contains", "view-root", "pointer");
    graph.add_predicate("contains", "window", "label");
    graph.add_predicate("contains", "view-root", "log-view");

    let stem = graph.create_typed("stem", "view-root", ());
    let bg = graph.create_typed("background.clouds", "background", ());
    let win = graph.create_typed("window.main", "window", ());
    let label = graph.create_static_bytes("label.welcome", "label", b"Where to?");
    let pointer = graph.create_typed("pointer.default", "pointer", [120u8, 64u8]);
    let log = graph.create_typed("window.log", "log-view", ());

    graph.link(stem, bg, "contains");
    graph.link(stem, win, "contains");
    graph.link(stem, log, "contains");
    graph.link(stem, pointer, "contains");
    graph.link(win, label, "contains");
}
