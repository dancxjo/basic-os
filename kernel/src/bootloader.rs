extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;

use limine::BaseRevision;
use limine::request::{FramebufferRequest, RequestsEndMarker, RequestsStartMarker};
