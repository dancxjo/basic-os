#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use launcher::LauncherApp;

userland::app_main!(LauncherApp);
