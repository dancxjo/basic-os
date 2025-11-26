//! ThingOS apps are `no_std` libraries that behave like tiny binaries. Each app implements
//! [`App`] with `init` and `tick` methods and is driven by the compositor runner. Apps talk
//! to the kernel only through [`AppContext`], which wraps the syscall/graph layers.
//!
//! Minimal example:
//! ```ignore
//! use userland::prelude::*;
//!
//! struct MyApp {
//!     window: WindowHandle,
//! }
//!
//! impl App for MyApp {
//!     fn init(ctx: &mut AppContext) -> Self {
//!         let window = ctx.create_window("My App");
//!         MyApp { window }
//!     }
//!
//!     fn tick(&mut self, ctx: &mut AppContext, tick: u64) {
//!         if tick % 4 == 0 {
//!             ctx.clear_window(&self.window);
//!             ctx.draw_text(&self.window, format_args!("Hello at {tick}\n"));
//!         }
//!     }
//! }
//!
//! app_main!(MyApp);
//! ```

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::{self, Write};

use crate::canon;
use crate::graph::{self, load_thing, update_thing, Thingable, Window};
use crate::ipc;
use crate::watch::{
    drain_app, poll_watch as poll_watch_event, register_app, watch_graph as watch_graph_filter,
    watch_journal as watch_journal_filter, AppEvent, EventFilter, ThingFilter, WatchId,
};
use uuid::Uuid;

pub trait App {
    fn init(ctx: &mut AppContext) -> Self;
    fn tick(&mut self, ctx: &mut AppContext, tick: u64);

    fn on_event(&mut self, _ctx: &mut AppContext, _ev: AppEvent) {}
}

pub trait AppRunner {
    fn tick(&mut self, tick: u64);
}

pub type DynApp = Box<dyn AppRunner>;

#[derive(Clone)]
pub struct WindowHandle {
    window: Uuid,
    pixmap: Uuid,
}

impl WindowHandle {
    pub fn window_id(&self) -> Uuid {
        self.window
    }

    pub fn pixmap_id(&self) -> Uuid {
        self.pixmap
    }
}

pub struct AppContext {
    compositor: Uuid,
    buffers: BTreeMap<Uuid, String>,
    window_pixmaps: BTreeMap<Uuid, Uuid>,
    window_counter: u64,
    app_id: usize,
}

impl AppContext {
    pub fn new(compositor: Uuid, app_id: usize) -> Self {
        Self {
            compositor,
            buffers: BTreeMap::new(),
            window_pixmaps: BTreeMap::new(),
            window_counter: 0,
            app_id,
        }
    }

    pub fn compositor_id(&self) -> Uuid {
        self.compositor
    }

    pub fn create_window(&mut self, title: &str) -> WindowHandle {
        let window_fields = Window {
            title: title.to_string(),
            x: 0,
            y: 0,
            width: 320,
            height: 200,
        };
        self.create_window_with(window_fields)
    }

    pub fn create_window_with(&mut self, window: Window) -> WindowHandle {
        let title = window.title.clone();
        let index = self.window_counter;
        self.window_counter = self.window_counter.wrapping_add(1);

        let window_name = format!("window-{title}-{index}");
        let pixmap_name = format!("pixmap-{title}-{index}");
        let window_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, window_name.as_bytes());
        let pixmap = Uuid::new_v5(&Uuid::NAMESPACE_OID, pixmap_name.as_bytes());

        let mut fields = window.to_fields();
        fields.insert(canon::NAME, graph::Value::Text(title.clone()));
        fields.insert(canon::TARGET, graph::Value::Uuid(pixmap));
        fields.insert(canon::STATUS, graph::Value::Symbol(canon::INIT));
        graph::fiat(Some(window_id), canon::WINDOW, fields);
        graph::that(window_id, canon::COMPOSED_BY, self.compositor, 0);

        self.window_pixmaps.insert(window_id, pixmap);

        WindowHandle {
            window: window_id,
            pixmap,
        }
    }

    pub fn clear_window(&mut self, win: &WindowHandle) {
        self.buffers.insert(win.window, String::new());
    }

    pub fn draw_text(&mut self, win: &WindowHandle, args: fmt::Arguments<'_>) {
        let buf = self.buffers.entry(win.window).or_insert_with(String::new);
        let _ = buf.write_fmt(args);
    }

    pub fn load_window(&self, win: &WindowHandle) -> Option<Window> {
        load_thing::<Window>(win.window)
    }

    pub fn update_window(&mut self, win: &WindowHandle, window: &Window) {
        update_thing(win.window, window);
    }

    pub fn begin_tick(&mut self) {
        self.buffers.clear();
    }

    pub fn flush(&mut self, tick: u64) {
        for (window, text) in self.buffers.iter() {
            if let Some(pixmap) = self.window_pixmaps.get(window) {
                ipc::emit_window_buffer_updated(*window, *pixmap, tick, text.as_bytes());
            }
        }
        self.buffers.clear();
    }

    pub fn watch_journal(&mut self, filter: EventFilter) -> WatchId {
        watch_journal_filter(self.app_id, filter)
    }

    pub fn watch_graph(&mut self, filter: ThingFilter) -> WatchId {
        watch_graph_filter(self.app_id, filter)
    }

    pub fn poll_watch(&mut self, watch: WatchId) -> Option<AppEvent> {
        poll_watch_event(self.app_id, watch)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = AppEvent> {
        drain_app(self.app_id)
    }
}

struct HostedApp<A: App> {
    app: A,
    ctx: AppContext,
}

impl<A: App> AppRunner for HostedApp<A> {
    fn tick(&mut self, tick: u64) {
        self.ctx.begin_tick();
        let mut drain = self.ctx.drain_events();
        while let Some(ev) = drain.next() {
            self.app.on_event(&mut self.ctx, ev);
        }
        self.app.tick(&mut self.ctx, tick);
        self.ctx.flush(tick);
    }
}

pub fn create_app<A: App + 'static>(compositor: Uuid) -> DynApp {
    let app_id = register_app();
    let mut ctx = AppContext::new(compositor, app_id);
    let app = A::init(&mut ctx);
    Box::new(HostedApp { app, ctx })
}

#[macro_export]
macro_rules! app_main {
    ($app_ty:ty) => {
        pub fn app_entry(
            compositor: uuid::Uuid,
        ) -> alloc::boxed::Box<dyn userland::app::AppRunner> {
            userland::app::create_app::<$app_ty>(compositor)
        }
    };
}
