#![no_std]
#![no_main]

extern crate alloc;
use userland::app_main;
use widget_host::WidgetHost;

app_main!(WidgetHost);
