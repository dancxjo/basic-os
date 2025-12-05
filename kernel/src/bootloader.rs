extern crate alloc;
use core::str;

use alloc::borrow::ToOwned;
use alloc::{boxed::Box, collections::BTreeMap};
use limine::memory_map::EntryType;
use limine::request::{HhdmRequest, MemoryMapRequest, ModuleRequest};
use log::{debug, info};
use spin::{Mutex, Once};
use x86_64::VirtAddr;

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

static MODULE_CACHE: Mutex<Option<BTreeMap<&'static str, &'static [u8]>>> = Mutex::new(None);

pub fn get_module(name: &str) -> Option<&'static [u8]> {
    ensure_module_cache();
    let guard = MODULE_CACHE.lock();
    guard.as_ref().and_then(|cache| {
        cache
            .iter()
            .find(|(k, _)| k.ends_with(name))
            .map(|(_, v)| *v)
    })
}

/// Return a snapshot of all loaded modules (path, data).
pub fn list_modules() -> alloc::vec::Vec<(&'static str, &'static [u8])> {
    ensure_module_cache();
    let guard = MODULE_CACHE.lock();
    guard
        .as_ref()
        .map(|cache| cache.iter().map(|(k, v)| (*k, *v)).collect())
        .unwrap_or_default()
}

fn ensure_module_cache() {
    let mut guard = MODULE_CACHE.lock();
    if guard.is_some() {
        return;
    }

    let mut map = BTreeMap::new();
    if let Some(response) = MODULE_REQUEST.get_response() {
        for module in response.modules() {
            if let Ok(path_str) = module.path().to_str() {
                let raw_addr = module.addr() as u64;
                let hhdm = get_hhdm_offset().as_u64();
                let base = if raw_addr >= hhdm {
                    raw_addr
                } else {
                    raw_addr + hhdm
                };
                log::info!(
                    "Module {}: addr={:#x} size={:#x} base={:#x}",
                    path_str,
                    raw_addr,
                    module.size(),
                    base
                );
                let ptr = base as *const u8;
                let len = module.size().try_into().unwrap();
                // SAFETY: We trust the bootloader (Limine) to provide valid module addresses and sizes.
                // The memory is guaranteed to be present and readable.
                let data = unsafe { core::slice::from_raw_parts(ptr, len) };
                map.insert(
                    Box::leak(path_str.to_owned().into_boxed_str()) as &str,
                    data,
                );
            }
        }
    }
    *guard = Some(map);
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub base: u64,
    pub len: u64,
    pub kind: &'static str,
}

pub fn collect_memory_regions() -> &'static [MemoryRegion] {
    static CACHE: Once<&'static [MemoryRegion]> = Once::new();
    static BUFF: Mutex<[MemoryRegion; 128]> = Mutex::new(
        [MemoryRegion {
            base: 0,
            len: 0,
            kind: "unknown",
        }; 128],
    );

    *CACHE.call_once(|| {
        let resp = MEMMAP_REQUEST
            .get_response()
            .expect("No memory map from Limine");
        let mut count = 0;
        let mut buff_guard = BUFF.lock();
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
            buff_guard[count] = MemoryRegion {
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
        // We need to leak the slice to return a static reference,
        // but since BUFF is static, we can just return a reference to the locked data?
        // No, we can't return a reference to data inside a Mutex guard.
        // However, since this is a bootloader info that is constant after initialization,
        // and we are in a single-threaded boot context (mostly), or we want to cache it.
        // The original code used static mut BUFF and returned a slice to it.
        // To be safe and keep the signature, we can leak a Boxed slice copy, or use a different approach.
        // Given the constraints and the original unsafe code, let's use a safe static with interior mutability
        // but we need to return &'static [].

        // Alternative: Use a static array wrapped in a Sync type that allows unsynchronized access if we guarantee it's initialized once.
        // Or just leak the vector.
        let mut vec = alloc::vec::Vec::with_capacity(count);
        for i in 0..count {
            vec.push(buff_guard[i]);
        }
        vec.leak()
    })
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

pub fn find_usable_stack_base() -> Option<u64> {
    for region in collect_memory_regions().iter() {
        if region.kind == "usable" && region.len >= (5 * 4096) {
            return Some(region.base);
        }
    }
    None
}
