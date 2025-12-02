use crate::graph::canon;
use crate::graph::canon::Symbol;
use crate::graph::types::{BundleId, GraphFiatRequest, KERNEL_BUNDLE_ID};
use crate::graph::{fiat_for_bundle, grant_capability};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::sync::atomic::{AtomicU64, Ordering};
use log::info;
use thing_abi::Value;
use uuid::Uuid;

static TASK_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Bundle type classification for the bundle lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleType {
    /// Device driver bundle
    Driver,
    /// User application bundle
    App,
    /// Window compositor bundle
    Compositor,
}

impl BundleType {
    /// Convert the bundle type to its corresponding symbol.
    pub fn to_symbol(self) -> Symbol {
        match self {
            BundleType::Driver => canon::DRIVER,
            BundleType::App => canon::APP,
            BundleType::Compositor => canon::COMPOSITOR,
        }
    }

    /// Attempt to infer bundle type from a module name.
    /// Uses suffix matching for more precise classification:
    /// - Names ending with "_driver" or "driver" are classified as Driver
    /// - Names ending with "_compositor" or "compositor" are classified as Compositor
    /// - Everything else defaults to App
    pub fn from_name(name: &str) -> Self {
        if name.ends_with("_driver") || name == "driver" || name.ends_with("driver") {
            BundleType::Driver
        } else if name.ends_with("_compositor")
            || name == "compositor"
            || name.ends_with("compositor")
        {
            BundleType::Compositor
        } else {
            BundleType::App
        }
    }
}

/// Create a new package node with proper type information and return its ID.
/// This is the primary entry point for creating packages (code modules).
pub fn create_package(name: &str, bundle_type: BundleType, version: Option<&str>) -> BundleId {
    let bundle_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    create_package_with_id(BundleId(bundle_id), name, bundle_type, version)
}

/// Create a package node with an explicit ID.
pub fn create_package_with_id(
    bundle_id: BundleId,
    name: &str,
    bundle_type: BundleType,
    version: Option<&str>,
) -> BundleId {
    let mut fields = BTreeMap::new();
    fields.insert(canon::ID, Value::Uuid(bundle_id.0));
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::TYPE, Value::Symbol(bundle_type.to_symbol()));
    fields.insert(canon::STATUS, Value::Symbol(canon::INIT));
    if let Some(v) = version {
        fields.insert(canon::VERSION, Value::Text(v.into()));
    }

    let req = GraphFiatRequest {
        id: Some(bundle_id.0),
        kind: canon::PACKAGE,
        labels: vec![canon::PACKAGE],
        fields,
    };

    // Use the package itself as owner (packages own themselves)
    fiat_for_bundle(bundle_id, req);
    bundle_id
}

/// Create a new task node (running instance) from a package.
///
/// This creates a new "Instance" of the given Bundle.
/// The returned ID represents the running actor, distinct from the static Bundle ID.
pub fn create_task(package_id: BundleId, name: &str, extra_labels: Vec<Symbol>) -> BundleId {
    let counter = TASK_COUNTER.fetch_add(1, Ordering::Relaxed);
    let unique_name = format!("{}-{}", name, counter);
    // The task ID is derived from the package ID but is unique per instance.
    let task_id = BundleId(Uuid::new_v5(&package_id.0, unique_name.as_bytes()));
    let mut fields = BTreeMap::new();
    fields.insert(canon::ID, Value::Uuid(task_id.0));
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::STATUS, Value::Symbol(canon::INIT));

    let mut labels = vec![canon::TASK];
    labels.extend(extra_labels);

    // Create the Task node
    let req = GraphFiatRequest {
        id: Some(task_id.0),
        kind: canon::TASK,
        labels,
        fields,
    };

    // The task owns itself
    info!("Calling fiat_for_bundle in create_task");
    fiat_for_bundle(task_id, req);
    info!("Returned from fiat_for_bundle in create_task");

    // Link Task -> Package
    info!("Calling that_for_bundle in create_task");
    crate::graph::that_for_bundle(
        task_id,
        crate::graph::types::GraphThatRequest {
            src: task_id.0,
            pred: canon::INSTANCE_OF.into(),
            dst: package_id.0,
            revision_hint: 0,
            props: BTreeMap::new(),
        },
    );
    info!("Returned from that_for_bundle in create_task");

    task_id
}

/// Look up an existing bundle by name, returning its BundleId if it exists.
pub fn lookup_bundle(name: &str) -> Option<BundleId> {
    let expected_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    crate::graph::with_store(|store| {
        store.latest(&expected_id).map(|thing| {
            if thing.kind == canon::PACKAGE {
                Some(BundleId(thing.id))
            } else {
                None
            }
        })?
    })
}

/// Get or create a bundle by name. If the bundle exists, returns its ID.
/// If it doesn't exist, creates a new bundle with the inferred type.
///
/// Note: This function is not atomic. During kernel boot when bundles are
/// initialized single-threaded, this is safe. If used in a concurrent context,
/// callers should ensure proper synchronization.
pub fn get_or_create_bundle(name: &str) -> BundleId {
    if let Some(id) = lookup_bundle(name) {
        return id;
    }
    let bundle_type = BundleType::from_name(name);
    create_package(name, bundle_type, None)
}

/// Grant initial capabilities to a bundle for a target node.
/// This is used when launching a bundle to give it access to its initial resources.
pub fn grant_initial_capability(bundle: BundleId, target: Uuid, capability: Symbol) -> bool {
    let req = crate::graph::GrantCapabilityRequest {
        grantee: bundle.0,
        target,
        capability: crate::graph::canon::symbol_to_string(capability),
    };
    grant_capability(KERNEL_BUNDLE_ID, req)
}
