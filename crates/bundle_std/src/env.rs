use crate::sys;
use thingos_kernel_std::id::BundleId;

pub fn current_bundle() -> BundleId {
    BundleId(sys::get_self())
}
