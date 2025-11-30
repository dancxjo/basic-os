#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use task_list::TaskListApp;

userland::app_main!(TaskListApp);
