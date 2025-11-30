use core::fmt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ThingId(pub Uuid);

impl fmt::Display for ThingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ThingId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BundleId(pub Uuid);

impl fmt::Display for BundleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for BundleId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

pub const KERNEL_BUNDLE_ID: BundleId =
    BundleId(Uuid::from_u128(0xfeed_cafe_dead_beef_cafe_babe_0000_0001));

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TaskId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<usize> for TaskId {
    fn from(id: usize) -> Self {
        Self(id as u64)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PredId(pub u64);

impl fmt::Display for PredId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for PredId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}
