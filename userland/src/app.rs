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
//!     fn init(ctx: &mut AppContext<'_>) -> Self {
//!         let window = ctx.create_window("My App");
//!         MyApp { window }
//!     }
//!
//!     fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
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
use alloc::vec::Vec;
use core::fmt::{self, Write};

use crate::canon;
use crate::graph::{self, load_thing, update_thing, Value, Window};
use crate::watch::{AppEvent, EventFilter, ThingFilter, WatchId, WatchManager};
use uuid::Uuid;

pub trait App {
    fn init(ctx: &mut AppContext<'_>) -> Self;
    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64);

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, _ev: AppEvent) {}
}

pub trait AppRunner {
    fn app_id(&self) -> usize;
    fn tick(&mut self, watch_manager: &mut WatchManager, tick: u64);
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

pub struct AppState {
    compositor: Uuid,
    buffers: BTreeMap<Uuid, String>,
    bitmaps: BTreeMap<Uuid, Vec<u8>>,
    window_pixmaps: BTreeMap<Uuid, Uuid>,
    window_counter: u64,
    app_id: usize,
}

impl AppState {
    pub fn new(compositor: Uuid, app_id: usize) -> Self {
        Self {
            compositor,
            buffers: BTreeMap::new(),
            bitmaps: BTreeMap::new(),
            window_pixmaps: BTreeMap::new(),
            window_counter: 0,
            app_id,
        }
    }
}

pub struct AppContext<'a> {
    state: &'a mut AppState,
    pub watch_manager: &'a mut WatchManager,
}

impl<'a> AppContext<'a> {
    pub fn compositor_id(&self) -> Uuid {
        self.state.compositor
    }

    pub fn create_window(&mut self, title: &str) -> WindowHandle {
        let window_fields = Window {
            id: Uuid::nil(),
            title: title.to_string(),
            x: 0,
            y: 0,
            width: 320,
            height: 200,
            z: 0,
            visible: true,
            target: None,
        };
        self.create_window_with(window_fields)
    }

    pub fn create_window_with(&mut self, window: Window) -> WindowHandle {
        let mut window = window;
        let index = self.state.window_counter;
        self.state.window_counter = self.state.window_counter.wrapping_add(1);
        if window.x == 0 && window.y == 0 {
            let offset = (index as u64) * 24;
            window.x = offset;
            window.y = offset;
        }
        window.z = index as i64;
        window.visible = true;
        let title = window.title.clone();

        let window_name = format!("window-{title}-{index}");
        let pixmap_name = format!("pixmap-{title}-{index}");
        let window_id = crate::simple_uuid(window_name.as_bytes());
        let pixmap = crate::simple_uuid(pixmap_name.as_bytes());

        window.target = Some(pixmap);

        let mut fields = window.to_fields();
        fields.insert(canon::NAME, graph::Value::Text(title.clone()));
        fields.insert(canon::TARGET, graph::Value::Uuid(pixmap));
        fields.insert(canon::STATUS, graph::Value::Symbol(canon::INIT));
        graph::fiat(Some(window_id), canon::WINDOW, fields);
        graph::that(window_id, canon::COMPOSED_BY, self.state.compositor, 0);

        self.state.window_pixmaps.insert(window_id, pixmap);

        WindowHandle {
            window: window_id,
            pixmap,
        }
    }

    pub fn clear_window(&mut self, win: &WindowHandle) {
        self.state.buffers.insert(win.window, String::new());
    }

    pub fn draw_text(&mut self, win: &WindowHandle, args: fmt::Arguments<'_>) {
        let buf = self
            .state
            .buffers
            .entry(win.window)
            .or_insert_with(String::new);
        let _ = buf.write_fmt(args);
    }

    pub fn draw_bitmap(&mut self, win: &WindowHandle, data: &[u8]) {
        self.state.bitmaps.insert(win.window, data.to_vec());
    }

    pub fn load_window(&self, win: &WindowHandle) -> Option<Window> {
        load_thing::<Window>(win.window)
    }

    pub fn update_window(&mut self, win: &WindowHandle, window: &Window) {
        update_thing(win.window, window.clone());
    }

    pub fn begin_tick(&mut self) {
        self.state.buffers.clear();
    }

    pub fn flush(&mut self, tick: u64) {
        for (window, text) in self.state.buffers.iter() {
            if let Some(pixmap) = self.state.window_pixmaps.get(window) {
                let mut payload = graph::map();
                payload.insert(canon::SRC, Value::Uuid(*window));
                payload.insert(canon::TARGET, Value::Uuid(*pixmap));
                payload.insert(canon::REVISION, Value::U64(tick));
                payload.insert(canon::TEXT, Value::Bytes(text.as_bytes().to_vec()));
                payload.insert(canon::DIRTY, Value::Bool(true));
                payload.insert(canon::VISIBLE, Value::Bool(true));

                if let Some(bmp) = self.state.bitmaps.remove(window) {
                    payload.insert(canon::BITMAP, Value::Bytes(bmp));
                }

                graph::fiat(Some(*pixmap), canon::SURFACE, payload);
                graph::that(*window, canon::HAS_SURFACE, *pixmap, 0);
            }
        }
        self.state.buffers.clear();
        // Bitmaps are removed as they are consumed
    }

    pub fn watch_journal(&mut self, filter: EventFilter) -> WatchId {
        self.watch_manager
            .register_journal(self.state.app_id, filter)
    }

    pub fn watch_graph(&mut self, filter: ThingFilter) -> WatchId {
        self.watch_manager.register_graph(self.state.app_id, filter)
    }

    pub fn drain_events(&mut self) -> Vec<AppEvent> {
        self.watch_manager.drain_inbox(self.state.app_id)
    }
}

struct HostedApp<A: App> {
    app: A,
    state: AppState,
}

impl<A: App> HostedApp<A> {}

impl<A: App> AppRunner for HostedApp<A> {
    fn app_id(&self) -> usize {
        self.state.app_id
    }

    fn tick(&mut self, watch_manager: &mut WatchManager, tick: u64) {
        let state = &mut self.state;
        let app = &mut self.app;
        let mut ctx = AppContext {
            state,
            watch_manager,
        };

        ctx.begin_tick();
        for ev in ctx.drain_events() {
            app.on_event(&mut ctx, ev);
        }
        app.tick(&mut ctx, tick);
        ctx.flush(tick);
    }
}

pub fn create_app<A: App + 'static>(compositor: Uuid, watch_manager: &mut WatchManager) -> DynApp {
    let app_id = watch_manager.register_app();
    let mut state = AppState::new(compositor, app_id);
    let app = {
        let mut ctx = AppContext {
            state: &mut state,
            watch_manager,
        };
        A::init(&mut ctx)
    };
    Box::new(HostedApp { app, state })
}

#[macro_export]
macro_rules! app_main {
    ($app_ty:ty) => {
        #[no_mangle]
        pub extern "C" fn _start() -> ! {
            $crate::init_heap();

            let mut watch_manager = $crate::WatchManager::new();
            let compositor_id = $crate::uuid::Uuid::nil();

            let mut app_runner =
                $crate::app::create_app::<$app_ty>(compositor_id, &mut watch_manager);

            let mut tick = 0;
            loop {
                let app_id = app_runner.app_id();
                watch_manager.process_graph(&[app_id]);
                app_runner.tick(&mut watch_manager, tick);
                tick = tick.wrapping_add(1);

                // Simple busy wait for now to avoid burning CPU too hard if idle
                // Ideally we would sleep until an event arrives
                for _ in 0..1000 {
                    core::hint::spin_loop();
                }
            }
        }

        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo) -> ! {
            $crate::println!("App Panic: {}", info);
            loop {}
        }
    };
}
