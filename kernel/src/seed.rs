use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::thing::Thingable;
use thing_macros::Thing;

#[derive(Debug, Serialize, Deserialize, Thing)]
pub struct SeedBlob {
    pub bytes: Vec<u8>,
}
