use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::{info, warn};
use spin::Mutex as SpinMutex;
use x86_64::PhysAddr;
use x86_64::structures::paging::PhysFrame;

use crate::bootloader::{get_hhdm_offset, get_module, list_modules};
use crate::drivers::device;
use crate::graph::{self, BundleId, BundleType, canon};
use crate::mm::allocator::BootFrameAllocator;
use crate::task::executable::{create_user_page_table, jump_to_user, load_elf};
use crate::task::runtime;

#[derive(Clone)]
/// Representation of a discovered user module and its bundle wiring.
///
/// Lifecycle overview:
/// 1. The bootloader supplies a raw ELF *module*.
/// 2. We create a deterministic graph *bundle node* for it using a v5 UUID.
/// 3. `start_user_task` turns that bundle into an executing *task*.
/// 4. We apply initial *capabilities* that connect the bundle to device nodes.
struct UserModule {
    name: &'static str,
    bundle: BundleId,
    bundle_type: BundleType,
}

static USER_MODULES: SpinMutex<Option<Vec<UserModule>>> = SpinMutex::new(None);
static NEXT_USER_MODULE: AtomicUsize = AtomicUsize::new(0);

pub fn init_user_modules() {
    let mut drivers = Vec::new();
    let mut compositors = Vec::new();
    let mut selected_app: Option<&'static str> = None;
    let mut preferred_apps = Vec::new();

    for (name, data) in list_modules().into_iter() {
        if !data.starts_with(b"\x7FELF") {
            info!("Skipping non-ELF module '{}'", name);
            continue;
        }

        let bundle_type = BundleType::from_name(name);
        match bundle_type {
            BundleType::Driver => {
                info!("Queueing user module '{}'", name);
                drivers.push(name);
            }
            BundleType::Compositor => {
                info!("Queueing user module '{}'", name);
                compositors.push(name);
            }
            BundleType::App => {
                if preferred_app(name) {
                    preferred_apps.push(name);
                    if selected_app.is_none() {
                        info!("Queueing preferred app module '{}'", name);
                        selected_app = Some(name);
                    } else {
                        info!("Skipping extra preferred app module '{}' for now", name);
                    }
                } else {
                    info!("Skipping non-preferred app module '{}' for now", name);
                }
            }
        }
    }

    if compositors.len() > 1 {
        warn!(
            "Multiple compositor modules detected ({}): {:?}; launching in discovery order",
            compositors.len(),
            compositors
        );
    } else if compositors.is_empty() {
        warn!("No compositor module discovered; userland will run without a compositor");
    }

    if preferred_apps.is_empty() {
        warn!(
            "No preferred app (clouds/input_tester) discovered; userland will start without an app"
        );
    }

    if drivers.is_empty() {
        info!("No user-space drivers discovered; continuing without driver bundles");
    }

    let mut entries = Vec::new();
    // Launch drivers first, then compositor, then a single preferred app.
    for name in drivers.into_iter() {
        entries.push(create_user_module(name, BundleType::Driver));
    }
    for name in compositors.into_iter() {
        entries.push(create_user_module(name, BundleType::Compositor));
    }
    if let Some(app_name) = selected_app {
        entries.push(create_user_module(app_name, BundleType::App));
    }

    *USER_MODULES.lock() = Some(entries);
    NEXT_USER_MODULE.store(0, Ordering::Release);
}

pub fn user_module_count() -> usize {
    USER_MODULES.lock().as_ref().map(|v| v.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn start_user_task() {
    let module = next_user_module().unwrap_or_else(|| {
        info!("No remaining user modules to start; halting task.");
        loop {}
    });

    runtime::assign_current_bundle(module.bundle);
    grant_initial_capabilities(&module);

    let module_bytes = get_module(module.name).unwrap_or_else(|| {
        panic!("Module '{}' not found", module.name);
    });
    let frame_allocator = BootFrameAllocator::global();
    let (new_l4, mut new_mapper) = create_user_page_table(frame_allocator, get_hhdm_offset());
    let loaded = load_elf(module_bytes, new_l4, &mut new_mapper, frame_allocator)
        .expect("Failed to load ELF");
    info!(
        "User entry prepared for {}: rip={:#x} stack_top={:#x}",
        module.name,
        loaded.entry.as_u64(),
        loaded.stack_top.as_u64()
    );
    let new_table_frame = PhysFrame::containing_address(PhysAddr::new(
        new_l4 as *const _ as u64 - get_hhdm_offset().as_u64(),
    ));
    runtime::set_current_cr3(new_table_frame.start_address().as_u64());
    unsafe {
        jump_to_user(loaded.entry, loaded.stack_top, new_table_frame);
    }
}

fn next_user_module() -> Option<UserModule> {
    let guard = USER_MODULES.lock();
    let list = guard.as_ref()?;
    let idx = NEXT_USER_MODULE.fetch_add(1, Ordering::AcqRel);
    match list.get(idx).cloned() {
        Some(module) => Some(module),
        None => {
            warn!(
                "NEXT_USER_MODULE index {} exceeded discovered module count {}; halting launcher task",
                idx,
                list.len()
            );
            None
        }
    }
}

fn create_user_module(name: &'static str, bundle_type: BundleType) -> UserModule {
    let bundle = graph::create_bundle(name, bundle_type, None);

    info!(
        "Created bundle for '{}' with type {:?}, id={}",
        name, bundle_type, bundle
    );

    UserModule {
        name,
        bundle,
        bundle_type,
    }
}

fn preferred_app(name: &str) -> bool {
    name.contains("clouds") || name.contains("input_tester")
}

fn framebuffer_node_id() -> uuid::Uuid {
    device::device_uuid(device::FRAMEBUFFER_DEVICE_NAME)
}

fn keyboard_node_id() -> uuid::Uuid {
    device::device_uuid(device::KEYBOARD_DEVICE_NAME)
}

fn mouse_node_id() -> uuid::Uuid {
    device::device_uuid(device::MOUSE_DEVICE_NAME)
}

fn driver_caps_for_name(name: &str) -> Option<(uuid::Uuid, &'static [canon::Symbol])> {
    if name.contains("keyboard") {
        Some((
            keyboard_node_id(),
            &[canon::CAN_READ, canon::CAN_HANDLE_IRQ, canon::CAN_PORT_IO],
        ))
    } else if name.contains("mouse") {
        Some((
            mouse_node_id(),
            &[canon::CAN_READ, canon::CAN_HANDLE_IRQ, canon::CAN_PORT_IO],
        ))
    } else if name.contains("framebuffer") {
        Some((
            framebuffer_node_id(),
            &[canon::CAN_WRITE, canon::CAN_READ, canon::CAN_MMIO],
        ))
    } else {
        None
    }
}

fn grant_initial_capabilities(module: &UserModule) {
    match module.bundle_type {
        BundleType::Driver => {
            if let Some((device, caps)) = driver_caps_for_name(module.name) {
                grant_caps(module.bundle, device, caps);
            } else {
                info!("No initial capabilities mapped for driver {}", module.name);
            }
        }
        BundleType::Compositor => {
            let fb_id = framebuffer_node_id();
            grant_caps(
                module.bundle,
                fb_id,
                &[canon::CAN_WRITE, canon::CAN_READ, canon::CAN_MMIO],
            );
        }
        BundleType::App => {
            info!("App {} starting with minimal capabilities", module.name);
        }
    }
}

fn grant_caps(bundle: BundleId, target: uuid::Uuid, caps: &[canon::Symbol]) {
    for cap in caps {
        if graph::grant_initial_capability(bundle, target, *cap) {
            info!("Granted {:?} on {} to bundle {}", cap, target, bundle);
        }
    }
}
