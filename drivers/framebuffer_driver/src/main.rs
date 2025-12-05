#![no_std]
#![no_main]

use framebuffer_driver::FramebufferDriver;
use userland::app_main;

app_main!(FramebufferDriver);
