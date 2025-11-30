use crate::graph::canon;
use crate::graph::canon::Symbol;
use crate::graph::types::{BundleId, GraphFiatRequest, KERNEL_BUNDLE_ID};
use crate::graph::{fiat_for_bundle, grant_capability};
use alloc::collections::BTreeMap;
use alloc::vec;
use thing_abi::Value;
use uuid::Uuid;

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

/// Create a new bundle node with proper type information and return its ID.
/// This is the primary entry point for creating bundles with the full lifecycle.
pub fn create_bundle(name: &str, bundle_type: BundleType, version: Option<&str>) -> BundleId {
    let bundle_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    create_bundle_with_id(bundle_id, name, bundle_type, version)
}

/// Create a bundle node with an explicit ID.
/// Useful when the BundleId has already been determined externally.
pub fn create_bundle_with_id(
    bundle_id: BundleId,
    name: &str,
    bundle_type: BundleType,
    version: Option<&str>,
) -> BundleId {
    let mut fields = BTreeMap::new();
    fields.insert(canon::ID, Value::Uuid(bundle_id));
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::TYPE, Value::Symbol(bundle_type.to_symbol()));
    fields.insert(canon::STATUS, Value::Symbol(canon::INIT));
    if let Some(v) = version {
        fields.insert(canon::VERSION, Value::Text(v.into()));
    }

    let req = GraphFiatRequest {
        id: Some(bundle_id),
        kind: canon::BUNDLE,
        labels: vec![canon::BUNDLE],
        fields,
    };

    // Use the bundle itself as owner (bundles own themselves)
    fiat_for_bundle(bundle_id, req);
    bundle_id
}

/// Look up an existing bundle by name, returning its BundleId if it exists.
pub fn lookup_bundle(name: &str) -> Option<BundleId> {
    let expected_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    crate::graph::with_store(|store| {
        store.latest(&expected_id).map(|thing| {
            if thing.kind == canon::BUNDLE {
                Some(thing.id)
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
    create_bundle(name, bundle_type, None)
}

/// Grant initial capabilities to a bundle for a target node.
/// This is used when launching a bundle to give it access to its initial resources.
pub fn grant_initial_capability(bundle: BundleId, target: Uuid, capability: Symbol) -> bool {
    let req = crate::graph::GrantCapabilityRequest {
        grantee: bundle,
        target,
        capability: crate::graph::canon::symbol_to_string(capability),
    };
    grant_capability(KERNEL_BUNDLE_ID, req)
}
