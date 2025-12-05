#![no_std]
#![no_main]

use keyboard_driver::KeyboardDriver;
use userland::app_main;

app_main!(KeyboardDriver);
