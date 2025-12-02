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
/// 3. `start_user_task` creates a new *task* (instance) from that bundle.
/// 4. We apply initial *capabilities* that connect the bundle instance to device nodes.
struct UserModule {
    name: &'static str,
    bundle: BundleId,
    bundle_type: BundleType,
}

static USER_MODULES: SpinMutex<Option<Vec<UserModule>>> = SpinMutex::new(None);
static NEXT_USER_MODULE: AtomicUsize = AtomicUsize::new(0);

pub fn init_user_modules() {
    let mut entries = Vec::new();
    let mut init_found = false;

    for (name, data) in list_modules().into_iter() {
        if name.contains("init") && data.starts_with(b"\x7FELF") {
            info!("Found init module '{}'", name);
            entries.push(create_user_module(name, BundleType::App));
            init_found = true;
            break;
        }
    }

    if !init_found {
        warn!("Init module not found! Falling back to legacy discovery.");
        let mut drivers = Vec::new();
        let mut compositors = Vec::new();
        let mut selected_app: Option<&'static str> = None;

        for (name, data) in list_modules().into_iter() {
            if !data.starts_with(b"\x7FELF") {
                continue;
            }
            let bundle_type = BundleType::from_name(name);
            match bundle_type {
                BundleType::Driver => drivers.push(name),
                BundleType::Compositor => compositors.push(name),
                BundleType::App => {
                    if preferred_app(name) && selected_app.is_none() {
                        selected_app = Some(name);
                    }
                }
            }
        }

        for name in drivers {
            entries.push(create_user_module(name, BundleType::Driver));
        }
        for name in compositors {
            entries.push(create_user_module(name, BundleType::Compositor));
        }
        if let Some(app) = selected_app {
            entries.push(create_user_module(app, BundleType::App));
        }
    }

    *USER_MODULES.lock() = Some(entries);
    NEXT_USER_MODULE.store(0, Ordering::Release);
}

pub fn spawn_module(name: &str) -> bool {
    let mut static_name: Option<&'static str> = None;
    for (m_name, _) in list_modules().into_iter() {
        if m_name.contains(name) {
            static_name = Some(m_name);
            break;
        }
    }

    let static_name = match static_name {
        Some(s) => s,
        None => {
            warn!("Module '{}' not found in boot modules", name);
            return false;
        }
    };

    let bundle_type = BundleType::from_name(static_name);
    let module = create_user_module(static_name, bundle_type);

    {
        let mut guard = USER_MODULES.lock();
        if let Some(list) = guard.as_mut() {
            list.push(module);
        } else {
            return false;
        }
    }

    unsafe extern "C" {
        fn task_entry_trampoline();
    }
    let trampoline: extern "C" fn() =
        unsafe { core::mem::transmute(task_entry_trampoline as unsafe extern "C" fn()) };
    runtime::spawn_kernel(trampoline);
    true
}

pub fn user_module_count() -> usize {
    USER_MODULES.lock().as_ref().map(|v| v.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn start_user_task() {
    info!("start_user_task reached, calling next_user_module...");
    let module = next_user_module().unwrap_or_else(|| {
        info!("No remaining user modules to start; halting task.");
        loop {}
    });
    info!("next_user_module returned {:?}", module.name);

    // Create a Task node for this running instance
    let mut labels = Vec::new();
    if module.bundle_type == BundleType::Driver {
        labels.push(canon::DRIVER);
    }
    let task_id = graph::create_task(module.bundle, module.name, labels);

    if module.bundle_type == BundleType::Driver {
        if let Some((device_id, _)) = driver_caps_for_name(module.name) {
            graph::that_for_bundle(
                task_id,
                graph::GraphThatRequest {
                    src: task_id.0,
                    pred: canon::DRIVES.into(),
                    dst: device_id,
                    revision_hint: 0,
                    props: alloc::collections::BTreeMap::new(),
                },
            );
        }
    }

    runtime::assign_current_bundle(task_id);
    grant_initial_capabilities(&module, task_id);

    let module_bytes = get_module(module.name).unwrap_or_else(|| {
        panic!("Module '{}' not found", module.name);
    });

    // Verify module bytes are accessible
    let module_ptr = module_bytes.as_ptr();
    info!(
        "Module {} data at {:p} (len={:#x})",
        module.name,
        module_ptr,
        module_bytes.len()
    );

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
    let bundle = graph::create_package(name, bundle_type, None);

    info!(
        "Created package for '{}' with type {:?}, id={}",
        name, bundle_type, bundle
    );

    UserModule {
        name,
        bundle,
        bundle_type,
    }
}

fn preferred_app(name: &str) -> bool {
    name.contains("demo_app") || name.contains("input_tester")
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

fn grant_initial_capabilities(module: &UserModule, task_id: BundleId) {
    match module.bundle_type {
        BundleType::Driver => {
            if let Some((device, caps)) = driver_caps_for_name(module.name) {
                grant_caps(task_id, device, caps);
            } else {
                info!("No initial capabilities mapped for driver {}", module.name);
            }
        }
        BundleType::Compositor => {
            let fb_id = framebuffer_node_id();
            grant_caps(
                task_id,
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
