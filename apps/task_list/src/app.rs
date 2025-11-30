use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use userland::prelude::*;
use userland::{canon, AppEvent, ThingFilter};
use uuid::Uuid;

pub struct TaskListApp {
    window: WindowHandle,
    tasks: BTreeMap<Uuid, TaskInfo>,
    watch_id: WatchId,
}

struct TaskInfo {
    name: String,
    kind: String,
    status: String,
}

impl App for TaskListApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Task List");

        let watch_id = ctx.watch_graph(ThingFilter {
            kind: Some(canon::TASK),
            id: None,
        });

        TaskListApp {
            window,
            tasks: BTreeMap::new(),
            watch_id,
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = ev {
            if watch == self.watch_id {
                let name = thing
                    .fields
                    .get(&canon::NAME)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let kind = thing
                    .fields
                    .get(&canon::TYPE)
                    .and_then(|v| v.as_symbol())
                    .map(|s| String::from(s))
                    .unwrap_or_else(|| "Unknown".to_string());

                let status = thing
                    .fields
                    .get(&canon::STATUS)
                    .and_then(|v| v.as_symbol())
                    .map(|s| String::from(s))
                    .unwrap_or_else(|| "Unknown".to_string());

                self.tasks.insert(thing.id, TaskInfo { name, kind, status });
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        ctx.clear_window(&self.window);

        ctx.draw_text(
            &self.window,
            format_args!("{:<20} {:<10} {:<10}\n", "Name", "Type", "Status"),
        );
        ctx.draw_text(&self.window, format_args!("{:-<40}\n", ""));

        for task in self.tasks.values() {
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<10} {:<10}\n", task.name, task.kind, task.status),
            );
        }
    }
}
