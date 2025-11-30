#![no_std]
#![no_main]

extern crate alloc;

use app_clouds::CloudsApp;

userland::app_main!(CloudsApp);
