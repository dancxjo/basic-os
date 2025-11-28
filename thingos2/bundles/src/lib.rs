//! Graph-native bundle prototypes. Each bundle manipulates the shared graph
//! through kernel syscalls and never touches hardware directly.

use std::collections::BTreeMap;

use thingos2_core::{BundleId, EdgeId, GraphError, NodeId, NodePattern, Value, WatchEvent};
use thingos2_kernel::{Kernel, TaskId};

/// Minimal context passed to bundle entrypoints.
pub struct BundleContext<'k> {
    pub kernel: &'k mut Kernel,
    pub bundle: BundleId,
    pub task: TaskId,
}

/// A framebuffer driver projects a framebuffer surface into the graph.
pub struct FramebufferDriver {
    framebuffer: Option<NodeId>,
    surface: Option<NodeId>,
}

impl FramebufferDriver {
    pub fn new() -> Self {
        Self {
            framebuffer: None,
            surface: None,
        }
    }

    pub fn initialize(
        &mut self,
        ctx: &mut BundleContext<'_>,
        width: u64,
        height: u64,
    ) -> Result<(NodeId, NodeId), GraphError> {
        let mut fb_props = BTreeMap::new();
        fb_props.insert("width".to_string(), Value::U128(width as u128));
        fb_props.insert("height".to_string(), Value::U128(height as u128));
        let fb = ctx.kernel.syscall_graph_fiat_node(
            ctx.bundle,
            vec!["Framebuffer".to_string()],
            fb_props,
        )?;

        let mut surf_props = BTreeMap::new();
        surf_props.insert("usage".to_string(), Value::from("surface"));
        let surface = ctx.kernel.syscall_graph_fiat_node(
            ctx.bundle,
            vec!["Surface".to_string()],
            surf_props,
        )?;

        ctx.kernel.syscall_graph_link(
            ctx.bundle,
            "OWNS_SURFACE".to_string(),
            fb,
            surface,
            BTreeMap::new(),
        )?;

        self.framebuffer = Some(fb);
        self.surface = Some(surface);
        Ok((fb, surface))
    }

    pub fn surface(&self) -> Option<NodeId> {
        self.surface
    }
}

/// Input driver publishes key events into the graph.
pub struct KeyboardDriver;

impl KeyboardDriver {
    pub fn publish_key(
        &self,
        ctx: &mut BundleContext<'_>,
        key: &str,
    ) -> Result<NodeId, GraphError> {
        let props = BTreeMap::from([("key".to_string(), Value::from(key))]);
        ctx.kernel
            .syscall_graph_fiat_node(ctx.bundle, vec!["KeyEvent".to_string()], props)
    }
}

/// Compositor watches for window + surface nodes and asks the framebuffer driver
/// to compose them. The composition algorithm is intentionally abstract and is
/// represented by graph writes.
pub struct Compositor {
    watch_id: Option<thingos2_core::WatchId>,
    last_frame_edges: Vec<EdgeId>,
}

impl Compositor {
    pub fn new() -> Self {
        Self {
            watch_id: None,
            last_frame_edges: Vec::new(),
        }
    }

    pub fn register_surface_watch(
        &mut self,
        ctx: &mut BundleContext<'_>,
    ) -> Result<(), GraphError> {
        let pattern = NodePattern::default().with_label("Surface");
        let watch_id = ctx.kernel.syscall_watch_register(ctx.bundle, pattern)?;
        self.watch_id = Some(watch_id);
        Ok(())
    }

    pub fn poll(&mut self, ctx: &mut BundleContext<'_>) -> Vec<WatchEvent> {
        ctx.kernel.syscall_watch_poll(ctx.bundle)
    }

    pub fn publish_frame_request(
        &mut self,
        ctx: &mut BundleContext<'_>,
        target_fb: NodeId,
        surface: NodeId,
    ) -> Result<EdgeId, GraphError> {
        let mut props = BTreeMap::new();
        props.insert("kind".to_string(), Value::from("HAS_SURFACE"));
        let edge = ctx.kernel.syscall_graph_link(
            ctx.bundle,
            "COMPOSES".to_string(),
            target_fb,
            surface,
            props,
        )?;
        self.last_frame_edges.push(edge);
        Ok(edge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_driver_projects_resources() {
        let mut kernel = Kernel::new();
        let (bundle, task) = kernel.launch_bundle("framebuffer");
        let mut driver = FramebufferDriver::new();
        let mut ctx = BundleContext {
            kernel: &mut kernel,
            bundle,
            task,
        };
        let (fb, surface) = driver.initialize(&mut ctx, 800, 600).unwrap();
        assert_eq!(driver.surface(), Some(surface));

        let pattern = NodePattern::default().with_label("Framebuffer");
        let nodes = ctx
            .kernel
            .syscall_graph_get_nodes(bundle, &pattern)
            .unwrap();
        assert_eq!(nodes, vec![fb]);
    }

    #[test]
    fn compositor_watches_surfaces_and_links_frames() {
        let mut kernel = Kernel::new();
        let (fb_bundle, fb_task) = kernel.launch_bundle("framebuffer");
        let mut fb_driver = FramebufferDriver::new();
        let mut fb_ctx = BundleContext {
            kernel: &mut kernel,
            bundle: fb_bundle,
            task: fb_task,
        };
        let (fb, surface) = fb_driver.initialize(&mut fb_ctx, 640, 480).unwrap();

        let (comp_bundle, comp_task) = fb_ctx.kernel.launch_bundle("compositor");
        fb_ctx
            .kernel
            .grant_capability_edge(
                fb_bundle,
                comp_bundle,
                "CAN_READ",
                fb,
                BTreeMap::new(),
                None,
            )
            .unwrap();
        fb_ctx
            .kernel
            .grant_capability_edge(
                fb_bundle,
                comp_bundle,
                "CAN_READ",
                surface,
                BTreeMap::new(),
                None,
            )
            .unwrap();
        let mut link_props = BTreeMap::new();
        link_props.insert("kind".to_string(), Value::from("COMPOSES"));
        fb_ctx
            .kernel
            .grant_capability_edge(
                fb_bundle,
                comp_bundle,
                "CAN_LINK",
                fb,
                link_props,
                Some(surface),
            )
            .unwrap();
        let mut compositor = Compositor::new();
        let mut comp_ctx = BundleContext {
            kernel: fb_ctx.kernel,
            bundle: comp_bundle,
            task: comp_task,
        };
        compositor.register_surface_watch(&mut comp_ctx).unwrap();

        let mut dirty_props = BTreeMap::new();
        dirty_props.insert("dirty".to_string(), Value::Bool(true));
        comp_ctx
            .kernel
            .syscall_graph_set_props(fb_bundle, surface, dirty_props)
            .unwrap();

        let events = compositor.poll(&mut comp_ctx);
        assert_eq!(events.len(), 1); // property change triggers watch visibility

        let edge = compositor
            .publish_frame_request(&mut comp_ctx, fb, surface)
            .expect("link composition");
        assert!(!comp_ctx.kernel.syscall_watch_poll(comp_bundle).is_empty());
        assert!(edge.0 >= 2);
    }

    #[test]
    fn keyboard_driver_creates_events() {
        let mut kernel = Kernel::new();
        let (bundle, task) = kernel.launch_bundle("keyboard");
        let driver = KeyboardDriver;
        let mut ctx = BundleContext {
            kernel: &mut kernel,
            bundle,
            task,
        };
        let node = driver.publish_key(&mut ctx, "A").unwrap();

        let pattern = NodePattern::default().with_label("KeyEvent");
        let visible = ctx
            .kernel
            .syscall_graph_get_nodes(bundle, &pattern)
            .unwrap();
        assert_eq!(visible, vec![node]);
    }
}
