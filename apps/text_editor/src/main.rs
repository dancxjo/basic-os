#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

extern crate alloc;

use text_editor::TextEditor;

userland::app_main!(TextEditor);
