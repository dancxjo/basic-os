use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use core::option::Option::{self, Some, None};
use userland::prelude::*;
use userland::{canon, AppEvent};

pub struct GraphViewerApp {
    window: WindowHandle,
}

struct GraphTaskInfo {
    name: String,
    role: String,
    state: String,
}

struct GraphWindowInfo {
    title: String,
    owner: String,
    active: bool,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct GraphDeviceInfo {
    kind: String,
    mode: String,
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
        ctx.draw_text(&self.window, format_args!("Graph Inspector (read-only)\n\n"));

        // Tasks
        let tasks = self.list_tasks();
        ctx.draw_text(&self.window, format_args!("Tasks:\n"));
        ctx.draw_text(&self.window, format_args!("{:<20} {:<10} {:<10}\n", "Name", "Role", "State"));
        ctx.draw_text(&self.window, format_args!("----------------------------------------\n"));
        for task in tasks {
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<10} {:<10}\n", task.name, task.role, task.state),
            );
        }
        ctx.draw_text(&self.window, format_args!("\n"));

        // Windows
        let windows = self.list_windows();
        ctx.draw_text(&self.window, format_args!("Windows:\n"));
        ctx.draw_text(&self.window, format_args!("{:<20} {:<15} {:<7} {:<15}\n", "Title", "Owner", "Active", "Pos/Size"));
        ctx.draw_text(&self.window, format_args!("------------------------------------------------------------\n"));
        for win in windows {
            let pos_size = format!("({},{} {}x{})", win.x, win.y, win.width, win.height);
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<15} {:<7} {:<15}\n", win.title, win.owner, if win.active { "yes" } else { "no" }, pos_size),
            );
        }
        ctx.draw_text(&self.window, format_args!("\n"));

        // Devices
        let devices = self.list_devices();
        ctx.draw_text(&self.window, format_args!("Devices:\n"));
        ctx.draw_text(&self.window, format_args!("{:<20} {:<10}\n", "Kind", "Mode"));
        ctx.draw_text(&self.window, format_args!("------------------------------\n"));
        for dev in devices {
            ctx.draw_text(
                &self.window,
                format_args!("{:<20} {:<10}\n", dev.kind, dev.mode),
            );
        }
    }
}

impl GraphViewerApp {
    fn list_tasks(&self) -> Vec<GraphTaskInfo> {
        let things = userland::graph::find_by_kind(&String::from(canon::BUNDLE));
        things.into_iter().map(|t| {
            let name = t.fields.get(&canon::NAME).and_then(|v| v.as_text()).map(|s| s.to_string()).unwrap_or_else(|| "Unknown".to_string());
            let role = t.fields.get(&canon::TYPE).and_then(|v| v.as_symbol()).map(|s| String::from(s)).unwrap_or_else(|| "APP".to_string());
            let state = t.fields.get(&canon::STATUS).and_then(|v| v.as_symbol()).map(|s| String::from(s)).unwrap_or_else(|| "running".to_string());
            GraphTaskInfo { name, role, state }
        }).collect()
    }

    fn list_windows(&self) -> Vec<GraphWindowInfo> {
        let things = userland::graph::find_by_kind(&String::from(canon::WINDOW));
        things.into_iter().map(|t| {
            let title = t.fields.get(&canon::TITLE).and_then(|v| v.as_text()).map(|s| s.to_string()).unwrap_or_else(|| "Untitled".to_string());
            let owner_uuid = t.fields.get(&canon::OWNER).and_then(|v| v.as_uuid());
            let owner = if let Some(_uuid) = owner_uuid {
                "task".to_string() 
            } else {
                "system".to_string()
            };
            
            let active = t.fields.get(&canon::ACTIVE).and_then(|v| v.as_bool()).unwrap_or(false);
            let x = t.fields.get(&canon::X).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = t.fields.get(&canon::Y).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let width = t.fields.get(&canon::WIDTH).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let height = t.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64()).unwrap_or(0) as u32;

            GraphWindowInfo { title, owner, active, x, y, width, height }
        }).collect()
    }

    fn list_devices(&self) -> Vec<GraphDeviceInfo> {
        let things = userland::graph::find_by_kind(&String::from(canon::DEVICE));
        things.into_iter().map(|t| {
             let kind = if t.labels.contains(&canon::KEYBOARD_DEVICE) { "keyboard" }
             else if t.labels.contains(&canon::MOUSE_DEVICE) { "mouse" }
             else if t.labels.contains(&canon::FRAMEBUFFER_DEVICE) { "framebuffer" }
             else { "other" };

             let mode = "native"; 

             GraphDeviceInfo { kind: kind.to_string(), mode: mode.to_string() }
        }).collect()
    }
}
