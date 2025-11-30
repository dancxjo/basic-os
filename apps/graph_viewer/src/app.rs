use alloc::string::{String, ToString};
use alloc::vec::Vec;
use userland::prelude::*;
use userland::{canon, AppEvent};

pub struct GraphViewerApp {
    window: WindowHandle,
}

struct TaskInfo {
    name: String,
    role: String,
    state: String,
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
    }
}

impl GraphViewerApp {
    fn list_tasks_from_graph(&self) -> Vec<TaskInfo> {
        let things = userland::graph::find_by_kind(&String::from(canon::BUNDLE));
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
}
