#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use graph_viewer::GraphViewerApp;

userland::app_main!(GraphViewerApp);
