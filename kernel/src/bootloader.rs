extern crate alloc;
use core::str;

#[used]
static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

use alloc::borrow::ToOwned;
use alloc::{boxed::Box, collections::BTreeMap};
use core::sync::atomic::{AtomicBool, Ordering};
use limine::request::ModuleRequest;

use crate::graph::Graph;

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

pub fn thingify_boot_modules(graph: &mut Graph) {
    graph.add_kind("module", "A boot module loaded via Limine");
    graph.add_kind("boot_modules", "Collection of boot modules");
    graph.add_predicate("contains", "boot_modules", "module");

    let root_id = graph.create_typed("boot.modules", "boot_modules", ());

    if let Some(response) = MODULE_REQUEST.get_response() {
        for (i, module) in response.modules().into_iter().enumerate() {
            let name = module.path().to_str().unwrap_or("unnamed");
            let data_ptr = module.addr() as *const u8;
            let size = module.size().try_into().unwrap();
            let data = unsafe { core::slice::from_raw_parts(data_ptr, size) };

            let boxed_name =
                alloc::boxed::Box::leak(alloc::string::String::from(name).into_boxed_str());
            let module_id = graph.create_static_bytes(boxed_name, "module", data);
            graph.link(root_id, module_id, "contains");
        }
    }
}
