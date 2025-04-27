extern crate alloc;
use core::str;

use alloc::borrow::ToOwned;
use alloc::{boxed::Box, collections::BTreeMap};
use core::sync::atomic::{AtomicBool, Ordering};
use limine::memory_map::EntryType;
use limine::request::{HhdmRequest, MemoryMapRequest, ModuleRequest};
use log::{debug, info};
use x86_64::VirtAddr;

use crate::paging::MemoryRegion;

#[used]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

pub fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    VirtAddr::new(resp.offset())
}

#[used]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();
#[used]
pub static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

static mut MODULE_CACHE: Option<BTreeMap<&'static str, &'static [u8]>> = None;
static INIT: AtomicBool = AtomicBool::new(false);

pub fn get_module(name: &str) -> Option<&'static [u8]> {
    unsafe {
        if !INIT.load(Ordering::Acquire) {
            let mut map = BTreeMap::new();
            if let Some(response) = MODULE_REQUEST.get_response() {
                for module in response.modules() {
                    if let Ok(path_str) = module.path().to_str() {
                        let ptr = module.addr() as *const u8;
                        let len = module.size().try_into().unwrap();
                        let data = core::slice::from_raw_parts(ptr, len);
                        map.insert(
                            Box::leak(path_str.to_owned().into_boxed_str()) as &str,
                            data,
                        );
                    }
                }
            }
            MODULE_CACHE = Some(map);
            INIT.store(true, Ordering::Release);
        }
        #[allow(static_mut_refs)]
        MODULE_CACHE.as_ref().and_then(|cache| {
            cache
                .iter()
                .find(|(k, _)| k.ends_with(name))
                .map(|(_, v)| *v)
        })
    }
}

pub fn collect_memory_regions() -> &'static [MemoryRegion] {
    static mut CACHE: Option<&'static [MemoryRegion]> = None;
    static mut BUFF: [MemoryRegion; 128] = [MemoryRegion {
        base: 0,
        len: 0,
        kind: "unknown",
    }; 128];
    unsafe {
        if let Some(r) = CACHE {
            return r;
        }
        let resp = MEMMAP_REQUEST
            .get_response()
            .expect("No memory map from Limine");
        let mut count = 0;
        for e in resp.entries().iter() {
            let kind = match e.entry_type {
                EntryType::USABLE => "usable",
                EntryType::RESERVED => "reserved",
                EntryType::ACPI_RECLAIMABLE => "acpi_reclaimable",
                EntryType::ACPI_NVS => "acpi_nvs",
                EntryType::BAD_MEMORY => "bad_memory",
                EntryType::BOOTLOADER_RECLAIMABLE => "bootloader_reclaimable",
                EntryType::FRAMEBUFFER => "framebuffer",
                _ => "unknown",
            };
            BUFF[count] = MemoryRegion {
                base: e.base,
                len: e.length,
                kind,
            };
            debug!(
                "region {}: base={:#x} len={:#x} kind={}",
                count, e.base, e.length, kind
            );
            count += 1;
        }
        let slice = &BUFF[..count];
        CACHE = Some(slice);
        debug!("total regions = {}", count);
        slice
    }
}

/// Debug helper to print detected memory regions
pub fn print_memory_regions() {
    info!("print_memory_regions start");
    for (i, region) in collect_memory_regions().iter().enumerate() {
        info!(
            "region[{}] base={:#x} len={:#x} kind={}",
            i, region.base, region.len, region.kind
        );
    }
    info!("print_memory_regions end");
}
