use alloc::string::{String, ToString};
use alloc::vec::Vec;
use userland::prelude::*;
use userland::{canon, AppEvent, NodePattern};

pub struct GraphViewerApp {
    window: WindowHandle,
}

struct TaskInfo {
    name: String,
    role: String,
    state: String,
}

struct BundleInfo {
    name: String,
    kind: String,
    version: String,
}

struct DocumentInfo {
    name: String,
    dirty: bool,
}

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Graph Inspector");
        GraphViewerApp { window }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, _ev: AppEvent) {
        // For now, we don't handle specific events, just redraw on tick
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        ctx.clear_window(&self.window);

        // Header
        ctx.draw_text(
            &self.window,
            format_args!("Graph Inspector (read-only)\n\n"),
        );

        // Bundles
        let bundles = self.list_bundles_from_graph();
        ctx.draw_text(&self.window, format_args!("Bundles:\n"));
        ctx.draw_text(
            &self.window,
            format_args!("{:<20} {:<10} {:<10}\n", "Name", "Type", "Version"),
        );
        ctx.draw_text(
            &self.window,
            format_args!("----------------------------------------\n"),
        );
        for bundle in bundles {
            ctx.draw_text(
                &self.window,
                format_args!(
                    "{:<20} {:<10} {:<10}\n",
                    bundle.name, bundle.kind, bundle.version
                ),
            );
        }
        ctx.draw_text(&self.window, format_args!("\n"));

        // Tasks
        let tasks = self.list_tasks_from_graph();
        ctx.draw_text(&self.window, format_args!("Tasks:\n"));
        ctx.draw_text(
            &self.window,
            format_args!("{:<20} {:<10} {:<10}\n", "Name", "Role", "State"),
        );
        ctx.draw_text(
            &self.window,
            format_args!("----------------------------------------\n"),
        );
        for task in tasks {
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<10} {:<10}\n", task.name, task.role, task.state),
            );
        }

        // Documents
        let docs = self.list_documents_from_graph();
        if !docs.is_empty() {
            ctx.draw_text(&self.window, format_args!("\nDocuments:\n"));
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<10}\n", "Name", "Dirty"),
            );
            ctx.draw_text(
                &self.window,
                format_args!("----------------------------------------\n"),
            );
            for doc in docs {
                let dirty_str = if doc.dirty { "yes" } else { "no" };
                ctx.draw_text(
                    &self.window,
                    format_args!("{:<20} {:<10}\n", doc.name, dirty_str),
                );
            }
        }
    }
}

impl GraphViewerApp {
    fn list_bundles_from_graph(&self) -> Vec<BundleInfo> {
        let mut pattern = NodePattern::default();
        pattern.labels.push(canon::PACKAGE);
        let things = userland::graph::get_nodes(pattern);
        things
            .into_iter()
            .map(|t| {
                let name = t
                    .fields
                    .get(&canon::NAME)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let kind = t
                    .fields
                    .get(&canon::TYPE)
                    .and_then(|v| v.as_symbol())
                    .map(|s| String::from(s))
                    .unwrap_or_else(|| "Unknown".to_string());
                let version = t
                    .fields
                    .get(&canon::VERSION)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "-".to_string());
                BundleInfo {
                    name,
                    kind,
                    version,
                }
            })
            .collect()
    }

    fn list_tasks_from_graph(&self) -> Vec<TaskInfo> {
        let mut pattern = NodePattern::default();
        pattern.labels.push(canon::TASK);
        let things = userland::graph::get_nodes(pattern);
        things
            .into_iter()
            .map(|t| {
                let name = t
                    .fields
                    .get(&canon::NAME)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let role = t
                    .fields
                    .get(&canon::TYPE)
                    .and_then(|v| v.as_symbol())
                    .map(|s| String::from(s))
                    .unwrap_or_else(|| "APP".to_string());
                let state = t
                    .fields
                    .get(&canon::STATUS)
                    .and_then(|v| v.as_symbol())
                    .map(|s| String::from(s))
                    .unwrap_or_else(|| "IN".to_string());
                TaskInfo { name, role, state }
            })
            .collect()
    }

    fn list_documents_from_graph(&self) -> Vec<DocumentInfo> {
        let mut pattern = NodePattern::default();
        pattern.labels.push(canon::DOCUMENT);
        let things = userland::graph::get_nodes(pattern);
        things
            .into_iter()
            .map(|t| {
                let name = t
                    .fields
                    .get(&canon::NAME)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let dirty = t
                    .fields
                    .get(&canon::DIRTY)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                DocumentInfo { name, dirty }
            })
            .collect()
    }
}
