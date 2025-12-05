#![no_std]
#![no_main]

use mouse_driver::MouseDriver;
use userland::app_main;

app_main!(MouseDriver);
