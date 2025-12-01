#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std as alloc;

use alloc::string::String;
use thing_abi::canon;
use thingos_bundle_std::graph::current_bundle_handle;
use thingos_kernel_std::id::PredId;

// Define predicates
// launch_count: 'L' 'C' 'T'
const PRED_LAUNCH_COUNT: PredId = PredId(canon(b'L', b'C', b'T').0 as u64);
// first_launched_at: 'F' 'L' 'A'
const PRED_FIRST_LAUNCHED_AT: PredId = PredId(canon(b'F', b'L', b'A').0 as u64);
// last_launched_at: 'L' 'L' 'A'
const PRED_LAST_LAUNCHED_AT: PredId = PredId(canon(b'L', b'L', b'A').0 as u64);

fn now_iso8601(count: u64) -> String {
    // Fake timestamp
    alloc::format!("2025-11-30T00:00:{:02}Z", count % 60)
}

pub fn app_main() -> ! {
    #[cfg(feature = "std")]
    userland::ensure_kernel_runtime();

    let bundle = current_bundle_handle();
    let root = bundle.get_root_thing();

    // 1. Read existing props
    let props = bundle
        .get_props(root)
        .unwrap_or_else(|_| thingos_bundle_std::graph::BundleProps {
            props: alloc::collections::BTreeMap::new(),
        });

    // 2. Compute new values
    let old_count = props.get_u64(PRED_LAUNCH_COUNT).unwrap_or(0);
    let new_count = old_count + 1;

    let now = now_iso8601(new_count);

    let first = props
        .get_text(PRED_FIRST_LAUNCHED_AT)
        .unwrap_or_else(|| now.clone());

    // 3. Write them back
    let _ = bundle.set_prop(root, PRED_LAUNCH_COUNT, new_count);
    let _ = bundle.set_prop(root, PRED_FIRST_LAUNCHED_AT, first.clone());
    let _ = bundle.set_prop(root, PRED_LAST_LAUNCHED_AT, now.clone());

    // Log
    thingos_bundle_std::sys::log(&alloc::format!(
        "Self-editing bundle launched {} times; first={}, last={}\n",
        new_count,
        first,
        now,
    ));

    loop {
        #[cfg(feature = "std")]
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
