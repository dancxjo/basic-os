#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod env;
pub mod id;
pub mod log;
pub mod prelude;
pub mod time;
