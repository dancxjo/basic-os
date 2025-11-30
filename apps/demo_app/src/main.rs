#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use demo_app::DemoApp;

userland::app_main!(DemoApp);
