extern crate alloc;
use core::str;

#[used]
static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

use alloc::borrow::ToOwned;
use alloc::{boxed::Box, collections::BTreeMap};
use core::sync::atomic::{AtomicBool, Ordering};
use limine::request::ModuleRequest;

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
