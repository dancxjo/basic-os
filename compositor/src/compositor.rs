use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::convert::TryInto;

use uuid::Uuid;
use widget_button::ButtonWidget;

use userland::graph;
use userland::graph::GraphPropsRequest;
use userland::widget_abi::WidgetAbi;
use userland::{
    canon, load_thing, println, AbiRequest, AppEvent, FramebufferGeometry, NodePattern, Surface,
    Thingable, Value, WatchId, WatchManager, Window,
};

use crate::bitmap::{decode_bmp, load_background, Bitmap};
use crate::cursor::{build_cursor_sprites, CursorKind, CursorSprites, CursorState};
use crate::layer::{build_wallpaper_scene, LayerKind, LayerState, WallpaperState};
use crate::layout::{self, AlignItems, FlexDirection, JustifyContent, LayoutItem, LayoutSpec};
use crate::mode::ModeSlot;
use crate::scene::{Scene, SceneItem};
use crate::types::{
    clamp_i32, Rect, Rgba, AUTO_TILE_MARGIN, AUTO_TILE_MIN_WINDOWS, AUTO_TILE_TOP_OFFSET,
    BORDER_3D_THICKNESS, BORDER_OUTER_THICKNESS, BORDER_THICKNESS, BTN_BORDER, BTN_FACE, BTN_GLYPH,
    CLEAR_COLOR, CLOSE_BUTTON_SIZE, COLOR_CURSOR_PRIMARY, COLOR_CURSOR_SHADOW, COLOR_TEXT, FONT_HEIGHT,
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, RESIZE_CORNER_SIZE, RESIZE_MARGIN,
    ROLE_CONTAINER_VERTICAL, ROLE_EDITOR_ROOT, ROLE_TOOLBAR, ROLE_TOOLBAR_BUTTON, SCROLLBAR_GAP,
    SCROLLBAR_MIN_THUMB, SCROLLBAR_THUMB_COLOR, SCROLLBAR_THUMB_HILIGHT, SCROLLBAR_THUMB_SHADOW,
    SCROLLBAR_TOTAL_RESERVE, SCROLLBAR_TRACK_COLOR, SCROLLBAR_WIDTH, SCROLL_STEP_LINE,
    TITLE_BAR_HEIGHT, TITLE_TEXT_LEFT_PAD, TITLE_TEXT_TOP_OFFSET, TOOLBAR_BUTTON_SIZE,
    TOOLBAR_BUTTON_SPACING, TOOLBAR_HEIGHT, Layout,
};
use crate::widget_manager::WidgetManager;
use crate::window::{
    close_button_rect, compute_window_layout, hit_test_resize, point_in_rect, rect_contains, Caret,
    ContentMetrics, DragKind, DragState, ResizeEdges, WindowLayout, WindowSurface,
};

use core::sync::atomic::{AtomicU64, Ordering};
static UUID_COUNTER: AtomicU64 = AtomicU64::new(0x10000);

fn next_uuid() -> Uuid {
    let id = UUID_COUNTER.fetch_add(1, Ordering::Relaxed);
    Uuid::from_u128(id as u128)
}

const COMPOSITOR_WIDGET: userland::Symbol = canon::canon(b'C', b'M', b'W');



pub struct FrameInfo {
    pub addr: u64,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u64,
}

pub trait FramebufferDevice<T> {
    fn geometry(&self) -> FramebufferGeometry;
    fn present(&mut self, frame: T);
    fn present_partial(&mut self, frame: T, _dirty_rect: Rect) {
        self.present(frame);
    }
    fn frame_info(&self) -> Option<FrameInfo> {
        None
    }
}

pub trait RendererBackend {
    type Output<'a>
    where
        Self: 'a;
    fn render<'a>(&'a mut self, scene: &Scene) -> Self::Output<'a>;
    fn render_partial<'a>(&'a mut self, scene: &Scene, _dirty_rect: Rect) -> Self::Output<'a> {
        self.render(scene)
    }
}

pub struct Compositor<F, R> {
    frame_no: u64,
    fb_device: F,
    renderer: R,
    windows: BTreeMap<Uuid, WindowSurface>,
    watch_surfaces: Option<WatchId>,
    watch_windows: Option<WatchId>,
    watch_mouse: Option<WatchId>,
    watch_keyboard: Option<WatchId>,
    watch_cursor: Option<WatchId>,
    watch_fb: Option<WatchId>,
    watch_widgets: Option<WatchId>,
    watch_mode: Option<WatchId>,
    watch_mode_defs: Option<WatchId>,
    watch_wallpaper: Option<WatchId>,
    watch_layer: Option<WatchId>,
    watch_style: Option<WatchId>,
    current_mode_node: Uuid,
    fb_id: Option<Uuid>,
    fb_dirty: bool,
    content_dirty: bool,
    auto_layout_done: bool,
    cursor: CursorState,
    cursor_sprites: CursorSprites,
    active_window: Option<Uuid>,
    active_mode: usize,
    active_place: Option<Uuid>,
    modes: [ModeSlot; 12],
    wallpapers: BTreeMap<Uuid, WallpaperState>,
    layers: BTreeMap<Uuid, LayerState>,
    saved_sky_geometry: BTreeMap<Uuid, Rect>,
    layout: Layout,
    bitmaps: BTreeMap<String, Arc<Bitmap>>,
    drag_state: Option<DragState>,
    alt_down: bool,
    shift_down: bool,
    state_node: Uuid,
    widget_manager: WidgetManager,
    debug_layout_mode: bool,
    debug_overlay_mode: bool,
    cursor_prev_rect: Option<Rect>,
}

impl<F, R> Compositor<F, R>
where
    R: RendererBackend,
    F: for<'a> FramebufferDevice<R::Output<'a>>,
{
    pub fn ensure_default_style(&mut self) {
        let pattern = NodePattern {
            labels: vec![canon::STYLE],
            ..Default::default()
        };
        let nodes = userland::graph::get_nodes(pattern);
        if nodes.is_empty() {
            let mut fields = BTreeMap::new();
            fields.insert(canon::KIND, Value::Symbol(canon::STYLE));
            fields.insert(canon::HEIGHT, Value::U64(32)); // title_bar_height
            fields.insert(canon::WIDTH, Value::I64(6)); // border_width/resize_margin (simplifying to just one for now)
            userland::fiat(None, canon::STYLE, fields);
        }
    }

    fn ingest_style(&mut self, thing: &userland::GraphThing) {
         if let Some(h) = thing.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64()) {
             self.layout.title_bar_height = h as usize;
         }
         // Can expand to colors and others later
         self.content_dirty = true;
         // Trigger comprehensive relayout
         self.enforce_place_layout();
         // self.update_scrollbars(); // Implicitly called in tick
    }

    pub fn new(fb_device: F, renderer: R) -> Self {
        let geo = fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        let background = load_background();
        let mut bitmaps = BTreeMap::new();
        bitmaps.insert("clouds.bmp".to_string(), background);
        let layout = Layout::default();
        let cursor_sprites = build_cursor_sprites();

        let state_node = userland::simple_uuid(b"CompositorState");
        let mut fields = userland::map();
        fields.insert(canon::NAME, Value::Text("CompositorState".into()));
        userland::fiat(Some(state_node), canon::COMPOSITOR, fields);

        let current_mode_node = userland::simple_uuid(b"CurrentMode");
        let mut fields = userland::map();
        fields.insert(canon::KIND, Value::Symbol(canon::CURRENT_MODE));
        fields.insert(canon::MODE_INDEX, Value::I64(0));
        userland::fiat(Some(current_mode_node), canon::CURRENT_MODE, fields);

        for i in 0..12 {
            let mode_node = userland::simple_uuid(alloc::format!("ModeF{}", i + 1).as_bytes());
            let mut fields = userland::map();
            fields.insert(canon::KIND, Value::Symbol(canon::MODE));
            fields.insert(canon::MODE_INDEX, Value::I64(i as i64));
            fields.insert(canon::NAME, Value::Text(alloc::format!("Mode F{}", i + 1)));

            // PRE-LINK F1 to "sky" place
            if i == 0 {
                let sky_place_id = userland::simple_uuid(b"sky");
                fields.insert(canon::MODE_PLACE, Value::Uuid(sky_place_id));
            }

            userland::fiat(Some(mode_node), canon::MODE, fields);
        }

        let mut modes = core::array::from_fn(|i| ModeSlot::new(i as u8));

        let mut layers = BTreeMap::new();
        let mut wallpapers = BTreeMap::new();

        // Sky Wallpaper
        let sky_layer_id = next_uuid();
        layers.insert(
            sky_layer_id,
            LayerState {
                id: sky_layer_id,
                kind: LayerKind::Image("clouds.bmp".to_string()),
                scroll_factor_x: 0.5,
                scroll_factor_y: 0.5,
                z_index: 0,
            },
        );

        let sky_place_id = userland::simple_uuid(b"sky");
        let sky_wallpaper_id = next_uuid();
        wallpapers.insert(
            sky_wallpaper_id,
            WallpaperState {
                id: sky_wallpaper_id,
                mode_node: None,
                place_id: Some(sky_place_id),
                layers: vec![sky_layer_id],
            },
        );

        Self {
            frame_no: 0,
            fb_device,
            renderer,
            windows: BTreeMap::new(),
            watch_surfaces: None,
            watch_windows: None,
            watch_mouse: None,
            watch_keyboard: None,
            watch_cursor: None,
            watch_fb: None,
            watch_widgets: None,
            watch_mode: None,
            watch_mode_defs: None,
            watch_wallpaper: None,
            watch_layer: None,
            watch_style: None,
            current_mode_node,
            fb_id: None,
            fb_dirty: false,
            content_dirty: true,
            auto_layout_done: false,
            cursor: CursorState::new(width, height),
            cursor_sprites,
            active_window: None,
            active_mode: 0,
            active_place: modes[0].place_id,
            modes,
            wallpapers,
            layers,
            saved_sky_geometry: BTreeMap::new(),
            layout,
            bitmaps,
            drag_state: None,
            alt_down: false,
            shift_down: false,
            state_node,
            widget_manager: WidgetManager::new(),
            debug_layout_mode: true,
            debug_overlay_mode: false,
            cursor_prev_rect: None,
        }
    }

    pub fn fb_device(&self) -> &F {
        &self.fb_device
    }

    pub fn fb_device_mut(&mut self) -> &mut F {
        &mut self.fb_device
    }

    pub fn renderer(&self) -> &R {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut R {
        &mut self.renderer
    }

    pub fn is_fb_dirty(&self) -> bool {
        self.fb_dirty
    }

    pub fn clear_fb_dirty(&mut self) {
        self.fb_dirty = false;
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.cursor.x = clamp_i32(self.cursor.x, 0, width.saturating_sub(1) as i32);
        self.cursor.y = clamp_i32(self.cursor.y, 0, height.saturating_sub(1) as i32);
        self.content_dirty = true;
    }

    fn update_graph_state(&self) {
        let mut fields = userland::map();

        // Window order (front to back)
        let mut order_ids = Vec::new();
        for id in self.ordered_window_ids().iter().rev() {
            order_ids.push(Value::Uuid(*id));
        }
        fields.insert(canon::ABOVE, Value::List(order_ids));

        // Active window
        if let Some(active) = self.active_window {
            fields.insert(canon::ACTIVE_WINDOW, Value::Uuid(active));
        } else {
            fields.insert(canon::ACTIVE_WINDOW, Value::Null);
        }

        userland::fiat(Some(self.state_node), canon::COMPOSITOR, fields);
    }

    fn init_wallpapers(&mut self) {
        // Create a wallpaper node for each mode
        // Create a wallpaper node for each mode
        for i in 0..12 {
            let mode_node = userland::simple_uuid(alloc::format!("ModeF{}", i + 1).as_bytes());
            let wallpaper_node =
                userland::simple_uuid(alloc::format!("WallpaperF{}", i + 1).as_bytes());

            let mut fields = userland::map();
            fields.insert(canon::KIND, Value::Symbol(canon::WALLPAPER));
            fields.insert(canon::MODE, Value::Uuid(mode_node));
            userland::fiat(Some(wallpaper_node), canon::WALLPAPER, fields);

            // SPECIAL CASE: F1 Mode runs Graph Viewer by default
            if i == 0 {
                // We still create the wallpaper as a fallback/background, but we add the APP property
                // to the Mode's Place (which is linked via ensure_mode_app logic, but here we set it on the place id)
                // Actually, init_wallpapers doesn't touch the Place directly.
                // The ModeF1 thing is created in new(), but the Place for it (sky) is set in new().
                
                // Let's set the 'app' property on the "sky" place which corresponds to Mode 0.
                let sky_place_id = userland::simple_uuid(b"sky");
                let mut app_fields = userland::map();
                app_fields.insert(canon::APP, Value::Text("graph_viewer".into()));
                userland::fiat(Some(sky_place_id), canon::PLACE, app_fields);
            }

            // Create default layers for the wallpaper
            // Layer 0: Background color (Sky)
            let layer0_node = userland::simple_uuid(alloc::format!("LayerF{}_0", i + 1).as_bytes());
            let mut fields = userland::map();
            fields.insert(canon::KIND, Value::Symbol(canon::LAYER));
            fields.insert(canon::WALLPAPER, Value::Uuid(wallpaper_node));
            fields.insert(canon::INDEX, Value::I64(0));
            fields.insert(canon::TYPE, Value::Symbol(canon::SOLID_COLOR));
            // Default sky color
            fields.insert(canon::COLOR, Value::U64(0xFF87CEEB));
            userland::fiat(Some(layer0_node), canon::LAYER, fields);

            // Layer 1: Clouds (Image)
            let layer1_node = userland::simple_uuid(alloc::format!("LayerF{}_1", i + 1).as_bytes());
            let mut fields = userland::map();
            fields.insert(canon::KIND, Value::Symbol(canon::LAYER));
            fields.insert(canon::WALLPAPER, Value::Uuid(wallpaper_node));
            fields.insert(canon::INDEX, Value::I64(1));
            fields.insert(canon::TYPE, Value::Symbol(canon::IMAGE));
            fields.insert(canon::IMAGE, Value::Text("clouds.bmp".to_string()));
            fields.insert(canon::SCROLL_X, Value::I64(500)); // Parallax factor * 1000
            fields.insert(canon::SCROLL_Y, Value::I64(100));
            userland::fiat(Some(layer1_node), canon::LAYER, fields);
        }
    }

    pub fn init_with_watches(
        watch_manager: &mut WatchManager,
        app_id: usize,
        fb_device: F,
        renderer: R,
    ) -> Self {
        let mut surface_pattern = NodePattern::default();
        surface_pattern.labels.push(canon::SURFACE);
        surface_pattern
            .props
            .insert(canon::DIRTY, Value::Bool(true));

        let mut surface_discovery = NodePattern::default();
        surface_discovery.labels.push(canon::SURFACE);

        let mut window_pattern = NodePattern::default();
        window_pattern.labels.push(canon::WINDOW);

        let mut cursor_pattern = NodePattern::default();
        cursor_pattern.labels.push(canon::CURSOR);

        let mut fb_pattern = NodePattern::default();
        fb_pattern.labels.push(canon::DISPLAY_FRAMEBUFFER);

        let mut mouse_pattern = NodePattern::default();
        mouse_pattern.labels.push(canon::INPUT_EVENT);

        let mut keyboard_pattern = NodePattern::default();
        keyboard_pattern.labels.push(canon::KEY_EVENT);

        let mut widget_pattern = NodePattern::default();
        widget_pattern.labels.push(canon::WIDGET);

        let mut mode_pattern = NodePattern::default();
        mode_pattern.labels.push(canon::CURRENT_MODE);

        let mut mode_def_pattern = NodePattern::default();
        mode_def_pattern.labels.push(canon::MODE);

        let mut wallpaper_pattern = NodePattern::default();
        wallpaper_pattern.labels.push(canon::WALLPAPER);

        let mut layer_pattern = NodePattern::default();
        layer_pattern.labels.push(canon::LAYER);

        let mut style_pattern = NodePattern::default();
        style_pattern.labels.push(canon::STYLE);

        let surface_watch = watch_manager.register_pattern(app_id, surface_pattern.clone());
        let window_watch = watch_manager.register_pattern(app_id, window_pattern.clone());
        let cursor_watch = watch_manager.register_pattern(app_id, cursor_pattern.clone());
        let fb_watch = watch_manager.register_pattern(app_id, fb_pattern.clone());
        let mouse_watch = watch_manager.register_pattern(app_id, mouse_pattern);
        let keyboard_watch = watch_manager.register_pattern(app_id, keyboard_pattern);
        let widget_watch = watch_manager.register_pattern(app_id, widget_pattern.clone());
        let mode_watch = watch_manager.register_pattern(app_id, mode_pattern);
        let mode_def_watch = watch_manager.register_pattern(app_id, mode_def_pattern.clone());
        let wallpaper_watch = watch_manager.register_pattern(app_id, wallpaper_pattern.clone());
        let layer_watch = watch_manager.register_pattern(app_id, layer_pattern.clone());
        let style_watch = watch_manager.register_pattern(app_id, style_pattern.clone());

        let mut comp = Self::new(fb_device, renderer);
        comp.init_wallpapers();
        comp.ensure_default_style();

        comp.watch_surfaces = Some(surface_watch);
        comp.watch_windows = Some(window_watch);
        comp.watch_mouse = Some(mouse_watch);
        comp.watch_keyboard = Some(keyboard_watch);
        comp.watch_cursor = Some(cursor_watch);
        comp.watch_fb = Some(fb_watch);
        comp.watch_widgets = Some(widget_watch);
        comp.watch_mode = Some(mode_watch);
        comp.watch_mode_defs = Some(mode_def_watch);
        comp.watch_wallpaper = Some(wallpaper_watch);
        comp.watch_layer = Some(layer_watch);
        comp.watch_style = Some(style_watch);

        for thing in userland::graph::get_nodes(wallpaper_pattern.clone()) {
            comp.ingest_wallpaper(&thing);
        }

        for thing in userland::graph::get_nodes(layer_pattern) {
            comp.ingest_layer(&thing);
        }

        for thing in userland::graph::get_nodes(style_pattern) {
            comp.ingest_style(&thing);
        }

        for thing in userland::graph::get_nodes(window_pattern) {
            if let Some(window) = Window::load(&thing) {
                comp.ingest_window(window);
            }
        }

        for thing in userland::graph::get_nodes(surface_discovery) {
            comp.ingest_surface(&thing);
        }

        for thing in userland::graph::get_nodes(widget_pattern) {
            comp.ingest_widget(&thing);
        }

        if let Some(cursor_node) = userland::graph::get_nodes(cursor_pattern)
            .into_iter()
            .next()
        {
            comp.ingest_cursor(&cursor_node);
        }

        for thing in userland::graph::get_nodes(mode_def_pattern) {
             if let Some(Value::I64(idx)) = thing.fields.get(&canon::MODE_INDEX) {
                let idx = (*idx).max(0).min(11) as usize;
                if let Some(place_id) = thing
                    .fields
                    .get(&canon::MODE_PLACE)
                    .and_then(|v| v.as_uuid())
                {
                    comp.modes[idx].place_id = Some(place_id);
                    // If we are currently in this mode, update active_place
                    if comp.active_mode == idx {
                         comp.active_place = Some(place_id);
                    }
                }
            }
        }

        comp
    }

    pub fn on_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } => {
                if Some(*watch) == self.watch_surfaces {
                    self.ingest_surface(thing);
                    self.content_dirty = true;
                } else if Some(*watch) == self.watch_mouse {
                    if thing.kind == canon::INPUT_EVENT {
                        self.ingest_input_event(thing);
                    }
                } else if Some(*watch) == self.watch_keyboard {
                    if thing.kind == canon::KEY_EVENT {
                        self.ingest_key_event(thing);
                    }
                } else if Some(*watch) == self.watch_windows {
                    if let Some(window) = Window::load(thing) {
                        self.ingest_window(window);
                        self.content_dirty = true;
                    }
                } else if Some(*watch) == self.watch_cursor {
                    self.ingest_cursor(thing);
                } else if Some(*watch) == self.watch_fb {
                    if thing.kind == canon::DISPLAY_FRAMEBUFFER {
                        self.fb_id = Some(thing.id);
                        self.fb_dirty = true;
                    }
                } else if Some(*watch) == self.watch_widgets {
                    self.ingest_widget(thing);
                    self.content_dirty = true;
                } else if Some(*watch) == self.watch_mode {
                    if thing.id == self.current_mode_node {
                        if let Some(Value::I64(idx)) = thing.fields.get(&canon::MODE_INDEX) {
                            let idx = (*idx).max(0).min(11) as usize;
                            if idx != self.active_mode {
                                self.active_mode = idx;
                                self.content_dirty = true;
                                self.active_place = self.modes[idx].place_id;
                                self.ensure_mode_app(idx);
                                self.update_graph_state();
                            }
                        }
                    }
                } else if Some(*watch) == self.watch_mode_defs {
                    if let Some(Value::I64(idx)) = thing.fields.get(&canon::MODE_INDEX) {
                        let idx = (*idx).max(0).min(11) as usize;
                        if let Some(place_id) = thing
                            .fields
                            .get(&canon::MODE_PLACE)
                            .and_then(|v| v.as_uuid())
                        {
                            self.modes[idx].place_id = Some(place_id);
                            if self.active_mode == idx {
                                self.active_place = Some(place_id);
                                self.content_dirty = true;
                            }
                        }
                    }
                } else if Some(*watch) == self.watch_wallpaper {
                    self.ingest_wallpaper(thing);
                    self.content_dirty = true;
                } else if Some(*watch) == self.watch_layer {
                    self.ingest_layer(thing);
                    self.content_dirty = true;
                }
            }
            AppEvent::Edge { .. } => {
                // No edge handling needed for now
            }
        }
    }

    fn ensure_scrollbar(&mut self, window_id: Uuid, metrics: &ContentMetrics) {
        let widget_id = self
            .windows
            .get(&window_id)
            .and_then(|w| w.scrollbar_widget_id);

        if metrics.content_height > metrics.viewport_height {
            let rect = metrics.scrollbar_rect();
            let thumb_offset = metrics.scrollbar_thumb_offset.unwrap_or(0);
            let thumb_height = metrics.scrollbar_thumb_rect.map(|r| r.height).unwrap_or(0);

            if let Some(id) = widget_id {
                let mut updates = BTreeMap::new();
                updates.insert(canon::X, Value::U64(rect.x as u64));
                updates.insert(canon::Y, Value::U64(rect.y as u64));
                updates.insert(canon::WIDTH, Value::U64(rect.width as u64));
                updates.insert(canon::HEIGHT, Value::U64(rect.height as u64));
                updates.insert(
                    canon::VIEWPORT_HEIGHT,
                    Value::I64(metrics.viewport_height as i64),
                );
                updates.insert(
                    canon::CONTENT_HEIGHT,
                    Value::I64(metrics.content_height as i64),
                );
                updates.insert(canon::SCROLL_Y, Value::I64(metrics.scroll_offset as i64));
                updates.insert(canon::MAX_SCROLL, Value::I64(metrics.max_scroll as i64));
                updates.insert(canon::THUMB_OFFSET, Value::I64(thumb_offset as i64));
                updates.insert(canon::THUMB_HEIGHT, Value::I64(thumb_height as i64));

                userland::graph::fiat(Some(id), canon::WIDGET, updates);
            } else {
                let mut fields = BTreeMap::new();
                fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
                fields.insert(canon::X, Value::U64(rect.x as u64));
                fields.insert(canon::Y, Value::U64(rect.y as u64));
                fields.insert(canon::WIDTH, Value::U64(rect.width as u64));
                fields.insert(canon::HEIGHT, Value::U64(rect.height as u64));

                fields.insert(
                    canon::cc('W', 'K'),
                    Value::Text(String::from("scrollbar_thumb")),
                );
                fields.insert(canon::PARENT, Value::Uuid(window_id));

                fields.insert(
                    canon::VIEWPORT_HEIGHT,
                    Value::I64(metrics.viewport_height as i64),
                );
                fields.insert(
                    canon::CONTENT_HEIGHT,
                    Value::I64(metrics.content_height as i64),
                );
                fields.insert(canon::SCROLL_Y, Value::I64(metrics.scroll_offset as i64));
                fields.insert(canon::MAX_SCROLL, Value::I64(metrics.max_scroll as i64));
                fields.insert(canon::THUMB_OFFSET, Value::I64(thumb_offset as i64));
                fields.insert(canon::THUMB_HEIGHT, Value::I64(thumb_height as i64));

                let id = userland::graph::fiat(None, canon::WIDGET, fields);

                if let Some(w) = self.windows.get_mut(&window_id) {
                    w.scrollbar_widget_id = Some(id);
                }
            }
        } else {
            if let Some(id) = widget_id {
                let mut updates = BTreeMap::new();
                updates.insert(canon::WIDTH, Value::U64(0));
                updates.insert(canon::HEIGHT, Value::U64(0));
                userland::graph::fiat(Some(id), canon::WIDGET, updates);
            }
        }
    }

    fn update_scrollbars(&mut self) {
        let ids: Vec<Uuid> = self.windows.keys().cloned().collect();
        for id in ids {
            let metrics = {
                if let Some(s) = self.windows.get(&id) {
                    if let Some(layout) = compute_window_layout(
                        s.window.x as i32,
                        s.window.y as i32,
                        s.window.width as i32,
                        s.window.height as i32,
                        self.layout.title_bar_height as i32,
                    ) {
                        Some(ContentMetrics::new(s, &layout))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(m) = metrics {
                self.ensure_scrollbar(id, &m);
            }
        }
    }

    fn get_cursor_rect(&self) -> Option<Rect> {
        if !self.cursor.visible {
            return None;
        }
        let icon = self.cursor_sprites.for_kind(self.cursor.kind);
        let w = icon.bitmap.width as u32;
        let h = icon.bitmap.height as u32;
        let hotspot = icon.hotspot;

        let x = self.cursor.x - hotspot.0;
        let y = self.cursor.y - hotspot.1;

        Some(Rect::new(x, y, w, h))
    }

    fn draw_root_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let x = 0;
        let y = 0;
        let w = fb_width;
        let h = fb_height;

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
        });

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
            color: self.layout.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                image: bmp.clone(),
                repeat: surface.repeat,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });
        }

        let has_widgets = self
            .widget_manager
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));

        if has_widgets {
            let layout = WindowLayout {
                title_x: 0,
                title_y: 0,
                title_w: 0,
                title_h: 0,
                client_x: x as i32,
                client_y: y as i32,
                client_w: w as i32,
                client_h: h as i32,
            };
            self.widget_manager.draw_widgets(
                scene,
                surface.window.id,
                &layout,
                layout.client_w,
                self.windows.get(&surface.window.id),
                &self.windows,
                None,
            );
        }

        scene.push(SceneItem::ClipPop);
    }

    fn draw_mode(&mut self, scene: &mut Scene, width: usize, height: usize) {
        self.draw_background(scene, width, height);

        let mode = &self.modes[self.active_mode];

        if let Some(root_id) = mode.root_window {
            if let Some(surface) = self.windows.get(&root_id).cloned() {
                self.draw_root_window(scene, &surface, width, height);
            }
        }

        for id in &mode.windows {
            if let Some(surface) = self.windows.get(id).cloned() {
                self.draw_window(scene, &surface, width, height);
            }
        }
    }

    fn enforce_place_layout(&mut self) {
        if let Some(place_id) = self.active_place {
            let fb_geo = self.fb_device.geometry();
            let width = fb_geo.width as u64;
            let height = fb_geo.height as u64;

            for surface in self.windows.values_mut() {
                if surface.window.is_place_root && surface.window.place_id == Some(place_id) {
                    if surface.window.x != 0
                        || surface.window.y != 0
                        || surface.window.width != width
                        || surface.window.height != height
                    {
                        surface.window.x = 0;
                        surface.window.y = 0;
                        surface.window.width = width;
                        surface.window.height = height;

                        let mut updates = BTreeMap::new();
                        updates.insert(canon::X, Value::U64(0));
                        updates.insert(canon::Y, Value::U64(0));
                        updates.insert(canon::WIDTH, Value::U64(width));
                        updates.insert(canon::HEIGHT, Value::U64(height));
                        userland::fiat(Some(surface.window.id), canon::WINDOW, updates);
                        self.content_dirty = true;
                    }
                }
            }
        }
    }

    pub fn tick(&mut self) {
        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        if width == 0 || height == 0 {
            return;
        }

        self.enforce_place_layout();
        self.ensure_active_window();
        self.update_cursor_kind();
        self.update_scrollbars();

        let cursor_new_rect = self.get_cursor_rect();
        let cursor_moved = cursor_new_rect != self.cursor_prev_rect;

        if !self.fb_dirty && !self.content_dirty && !cursor_moved {
            return;
        }

        let mut scene = Scene::new(width as u32, height as u32);
        scene.push(SceneItem::Clear { color: CLEAR_COLOR });

        self.draw_mode(&mut scene, width, height);

        self.draw_cursor(&mut scene, width, height);

        if self.fb_dirty || self.content_dirty {
            let frame = self.renderer.render(&scene);
            self.fb_device.present(frame);
            self.content_dirty = false;
        } else if cursor_moved {
            let dirty_rect = if let Some(prev) = self.cursor_prev_rect {
                if let Some(curr) = cursor_new_rect {
                    prev.union(curr)
                } else {
                    prev
                }
            } else {
                cursor_new_rect.unwrap_or(Rect::new(0, 0, 0, 0))
            };

            let frame = self.renderer.render_partial(&scene, dirty_rect);
            self.fb_device.present_partial(frame, dirty_rect);
        }

        self.cursor_prev_rect = cursor_new_rect;
        self.publish_frame_info();
        self.frame_no = self.frame_no.wrapping_add(1);
    }

    fn publish_frame_info(&mut self) {
        if let Some(fb_id) = self.fb_id {
            if let Some(info) = self.fb_device.frame_info() {
                let mut fields = BTreeMap::new();
                fields.insert(canon::KIND, Value::Symbol(canon::DISPLAY_FRAME));
                fields.insert(canon::SEQ, Value::U64(self.frame_no));
                fields.insert(canon::ADDR, Value::U64(info.addr));
                fields.insert(canon::WIDTH, Value::U64(info.width));
                fields.insert(canon::HEIGHT, Value::U64(info.height));
                fields.insert(canon::PITCH, Value::U64(info.pitch));
                fields.insert(canon::BPP, Value::U64(info.bpp));

                let frame_id = userland::fiat(None, canon::DISPLAY_FRAME, fields);
                userland::that(fb_id, canon::CURRENT_FRAME, frame_id, 0);
            }
        }
    }

    fn find_window_at(&self, x: i32, y: i32) -> Option<(Uuid, i32, i32)> {
        for win_id in self.ordered_window_ids().into_iter().rev() {
            let surface = self.windows.get(&win_id)?;
            if !surface.window.visible {
                continue;
            }
            let wx = surface.window.x as i32;
            let wy = surface.window.y as i32;
            let w = surface.window.width as i32;
            let h = surface.window.height as i32;

            if x >= wx && x < wx + w && y >= wy && y < wy + h {
                return Some((win_id, wx, wy));
            }
        }
        None
    }

    fn content_metrics_for_window(
        &self,
        window_id: Uuid,
    ) -> Option<(WindowLayout, ContentMetrics)> {
        let surface = self.windows.get(&window_id)?;
        let layout = compute_window_layout(
            surface.window.x as i32,
            surface.window.y as i32,
            surface.window.width as i32,
            surface.window.height as i32,
            self.layout.title_bar_height as i32,
        )?;
        let metrics = ContentMetrics::new(surface, &layout);
        Some((layout, metrics))
    }

    fn clamp_scroll_for(&mut self, window_id: Uuid) {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if let Some(surface) = self.windows.get_mut(&window_id) {
                if surface.scroll_y != metrics.scroll_offset {
                    surface.scroll_y = metrics.scroll_offset;
                }
            }
        } else if let Some(surface) = self.windows.get_mut(&window_id) {
            surface.scroll_y = 0;
        }
    }

    fn apply_window_rect_hint(&mut self, window_id: Uuid) {
        if let Some(surface) = self.windows.get_mut(&window_id) {
            if let Some(rect) = surface.window.window_rect {
                surface.caret.x = rect.x as i32;
                surface.caret.y = rect.y as i32;
                surface.caret.width = 2;
                surface.caret.height = rect.height as i32;
                surface.caret.visible = rect.visible;
            } else {
                surface.caret.visible = false;
            }
        }

        let Some(rect) = self
            .windows
            .get(&window_id)
            .and_then(|surface| surface.window.window_rect)
        else {
            return;
        };
        let Some((_, metrics)) = self.content_metrics_for_window(window_id) else {
            return;
        };
        if metrics.viewport_height <= 0 {
            return;
        }

        let current_scroll = self
            .windows
            .get(&window_id)
            .map(|surface| surface.scroll_y)
            .unwrap_or(0);
        let rect_top = clamp_i32(
            rect.y.clamp(0, i64::from(i32::MAX)).try_into().unwrap_or(0),
            0,
            i32::MAX,
        );
        let raw_height = rect.height.max(0);
        let rect_height = clamp_i32(
            raw_height
                .min(i64::from(i32::MAX))
                .try_into()
                .unwrap_or(FONT_HEIGHT as i32),
            FONT_HEIGHT as i32,
            i32::MAX,
        );
        let rect_bottom = rect_top.saturating_add(rect_height);
        let viewport_bottom = current_scroll + metrics.viewport_height;

        let mut desired_scroll = current_scroll;
        if rect_top < current_scroll {
            desired_scroll = rect_top;
        } else if rect_bottom > viewport_bottom {
            desired_scroll = rect_bottom - metrics.viewport_height;
        } else {
            return;
        }

        let clamped = metrics.clamp_scroll(desired_scroll);
        if let Some(surface) = self.windows.get_mut(&window_id) {
            if surface.scroll_y != clamped {
                surface.scroll_y = clamped;
            }
        }
    }

    fn set_scroll_offset(&mut self, window_id: Uuid, new_offset: i32) -> bool {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if metrics.max_scroll <= 0 {
                if let Some(surface) = self.windows.get_mut(&window_id) {
                    surface.scroll_y = 0;
                }
                return false;
            }
            let clamped = metrics.clamp_scroll(new_offset);
            if let Some(surface) = self.windows.get_mut(&window_id) {
                if surface.scroll_y != clamped {
                    surface.scroll_y = clamped;
                    return true;
                }
            }
        }
        false
    }

    fn scroll_window_by(&mut self, window_id: Uuid, delta: i32) -> bool {
        if delta == 0 {
            return false;
        }
        if let Some(surface) = self.windows.get(&window_id) {
            let new_offset = surface.scroll_y.saturating_add(delta);
            return self.set_scroll_offset(window_id, new_offset);
        }
        false
    }

    fn scroll_window_to_start(&mut self, window_id: Uuid) -> bool {
        self.set_scroll_offset(window_id, 0)
    }

    fn scroll_window_to_end(&mut self, window_id: Uuid) -> bool {
        if let Some((_, metrics)) = self.content_metrics_for_window(window_id) {
            if metrics.max_scroll > 0 {
                return self.set_scroll_offset(window_id, metrics.max_scroll);
            }
        }
        false
    }

    // Keyboard navigation for scrolling keeps UIs operable per WCAG 2.2 SC 2.1.1 (Keyboard).
    fn handle_scroll_key(&mut self, key: canon::Symbol) -> bool {
        let Some(active) = self.active_window else {
            return false;
        };
        let Some((_, metrics)) = self.content_metrics_for_window(active) else {
            return false;
        };
        if metrics.max_scroll <= 0 {
            return false;
        }
        let page = metrics.viewport_height.max(SCROLL_STEP_LINE);
        let delta = match key {
            k if k == canon::cc('A', 'U') => Some(-SCROLL_STEP_LINE),
            k if k == canon::cc('A', 'D') => Some(SCROLL_STEP_LINE),
            k if k == canon::cc('P', 'U') => Some(-page),
            k if k == canon::cc('P', 'D') => Some(page),
            _ => None,
        };
        if let Some(delta) = delta {
            return self.scroll_window_by(active, delta);
        }
        if key == canon::cc('H', 'M') {
            return self.scroll_window_to_start(active);
        }
        if key == canon::cc('E', 'D') {
            return self.scroll_window_to_end(active);
        }
        false
    }

    fn draw_close_button(&self, scene: &mut Scene, layout: &WindowLayout, surface: &WindowSurface) {
        let (btn_x, btn_y, btn_w, btn_h) = close_button_rect(layout);

        // Create temporary buffer
        let mut buffer = vec![0u8; (btn_w * btn_h * 4) as usize];
        let rect = userland::widget_abi::Rect {
            x: 0,
            y: 0,
            width: btn_w as u32,
            height: btn_h as u32,
        };

        ButtonWidget::draw(&surface.close_button_state, &mut buffer, rect);

        // Convert to u32 pixels for Bitmap
        let pixels: Vec<u32> = buffer
            .chunks(4)
            .map(|c| {
                let r = c[0] as u32;
                let g = c[1] as u32;
                let b = c[2] as u32;
                let a = c[3] as u32;
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect();

        let bitmap = Arc::new(Bitmap::new(btn_w as usize, btn_h as usize, pixels));

        scene.push(SceneItem::BlitImage {
            rect: Rect::new(btn_x, btn_y, btn_w as u32, btn_h as u32),
            image: bitmap,
            repeat: false,
            offset: (0, 0),
        });
    }

    fn ingest_surface(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::SURFACE {
            return;
        }
        let Some(surface) = Surface::load(thing) else {
            return;
        };
        let window_id = surface
            .window
            .or_else(|| thing.fields.get(&canon::SRC).and_then(Value::as_uuid));
        let Some(window_id) = window_id else {
            return;
        };

        let window = load_thing::<Window>(window_id).unwrap_or_else(|| default_window(window_id));
        if !self.windows.contains_key(&window_id) {
            let (title_bar_id, close_btn_id) = self.ensure_window_chrome(window_id, &window.title);
            let btn_state = widget_button::State {
                label: "".to_string(),
                target: "close_window".to_string(),
                pressed: false,
                hovered: false,
                focused: false,
                icon_name: Some("close".to_string()),
                show_label: true,
                bind_node: None,
                bind_index: None,
            };

            let surf = WindowSurface {
                window: window.clone(),
                surface_id: None,
                text: String::new(),
                bitmap: None,
                scroll_y: 0,
                scrollbar_widget_id: None,
                caret: Caret::default(),
                title_bar_id: Some(title_bar_id),
                close_button_id: close_btn_id,
                close_button_state: btn_state,
                close_button_bitmap: None,
                repeat: false,
            };
            self.windows.insert(window_id, surf);
        }

        let entry = self.windows.get_mut(&window_id).unwrap();
        if let Some(tile_mode) = window.tile_mode {
            entry.repeat = tile_mode;
        } else if let Some(tile_mode) = thing.fields.get(&canon::TILE_MODE).and_then(|v| v.as_bool()) {
            entry.repeat = tile_mode;
        }

        entry.window = window;
        entry.surface_id = Some(surface.id);
        entry.text = surface.text;

        if let Some(bytes) = surface.bitmap {
            if let Some(bmp) = decode_bmp(&bytes) {
                entry.bitmap = Some(Arc::new(bmp));
            }
        }

        self.bump_window(window_id);
        self.clamp_scroll_for(window_id);
        self.apply_window_rect_hint(window_id);
    }

    fn ordered_window_ids(&self) -> Vec<Uuid> {
        let mode = &self.modes[self.active_mode];
        let mut ids = Vec::new();
        if let Some(root) = mode.root_window {
            if let Some(w) = self.windows.get(&root) {
                if w.window.visible {
                    ids.push(root);
                }
            }
        }
        for &id in &mode.windows {
            if let Some(w) = self.windows.get(&id) {
                if w.window.visible {
                    ids.push(id);
                }
            }
        }
        ids
    }

    fn visible_window_ids(&self) -> Vec<Uuid> {
        self.ordered_window_ids()
            .into_iter()
            .filter(|id| {
                self.windows
                    .get(id)
                    .map(|surface| surface.surface_id.is_some())
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn on_mouse_event(&mut self, dx: i64, dy: i64, buttons: u64) {
        if dx != 0 || dy != 0 {
            // println!("Compositor move: dx={} dy={}", dx, dy);
        }

        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        let prev_buttons = self.cursor.buttons;
        self.cursor.update(dx, dy, buttons as u8, width, height);

        let left_down = (buttons & 1) != 0;
        let left_was_down = (prev_buttons & 1) != 0;
        let left_pressed = left_down && !left_was_down;
        let left_released = !left_down && left_was_down;

        if left_pressed {
            self.on_pointer_down();
        }

        if left_down {
            self.continue_drag();
            self.continue_widget_interaction();
        } else if left_released {
            if let Some(drag) = &self.drag_state {
                if let DragKind::CloseButton = drag.kind {
                    self.on_close_button_up(drag.window_id);
                }
            }
            self.drag_state = None;
            self.end_widget_interaction();
        } else {
            self.drag_state = None;
        }

        self.update_cursor_kind();
    }

    fn ingest_input_event(&mut self, thing: &userland::GraphThing) {
        // println!("Compositor ingest: {:?}", thing);
        if let Some(kind) = thing.fields.get(&canon::KIND).and_then(|v| v.as_symbol()) {
            if kind == canon::MOVE {
                let dx = thing
                    .fields
                    .get(&canon::DX)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let dy = thing
                    .fields
                    .get(&canon::DY)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let buttons = thing
                    .fields
                    .get(&canon::BUTTON)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                self.on_mouse_event(dx, dy, buttons);
            }
        }
    }

    fn on_close_button_up(&mut self, window_id: Uuid) {
        // Reset pressed state
        if let Some(surface) = self.windows.get_mut(&window_id) {
            surface.close_button_state.pressed = false;
        }

        // Check if still over button
        let (win_x, win_y, win_w, win_h) = if let Some(surface) = self.windows.get(&window_id) {
            (
                surface.window.x as i32,
                surface.window.y as i32,
                surface.window.width as i32,
                surface.window.height as i32,
            )
        } else {
            return;
        };

        if let Some(layout) = compute_window_layout(win_x, win_y, win_w, win_h, self.layout.title_bar_height as i32) {
            let close_rect = close_button_rect(&layout);
            if point_in_rect(self.cursor.x, self.cursor.y, close_rect) {
                println!("Close button clicked for window {}", window_id);
                let mut props = BTreeMap::new();
                props.insert(canon::VISIBLE, Value::Bool(false));
                self.update_window_props(window_id, props);
            }
        }
    }

    pub fn switch_mode(&mut self, new_mode_idx: usize) {
        if self.active_mode == new_mode_idx {
            return;
        }
        // Update the graph node instead of local state
        let mut fields = userland::map();
        fields.insert(canon::INDEX, Value::I64(new_mode_idx as i64));
        userland::fiat(Some(self.current_mode_node), canon::CURRENT_MODE, fields);
    }

    fn ingest_key_event(&mut self, thing: &userland::GraphThing) {
        let key = thing.fields.get(&canon::KEY).and_then(|v| v.as_symbol());
        let down = thing
            .fields
            .get(&canon::DOWN)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let scancode = thing.fields.get(&canon::SCANCODE).and_then(|v| v.as_u64());

        if let Some(key) = key {
            if scancode == Some(0x38)
                || (scancode.is_none()
                    && (key == canon::cc('A', 'L') || key == canon::cc('Y', 'L')))
            {
                self.alt_down = down;
            }

            if scancode == Some(0x2A)
                || scancode == Some(0x36)
                || (scancode.is_none() && key == canon::cc('S', 'F'))
            {
                self.shift_down = down;
            }

            if down {
                if self.alt_down && (key == canon::from_char('t') || key == canon::from_char('T')) {
                    self.tile_windows();
                } else if self.alt_down
                    && (key == canon::from_char('i') || key == canon::from_char('I'))
                {
                    self.debug_layout_mode = !self.debug_layout_mode;
                    self.fb_dirty = true;
                } else if self.alt_down
                    && (key == canon::from_char('o') || key == canon::from_char('O'))
                {
                    self.debug_overlay_mode = !self.debug_overlay_mode;
                    self.fb_dirty = true;
                } else if key.0 >= 0xF001 && key.0 <= 0xF00C {
                    let mode_idx = (key.0 - 0xF001) as usize;
                    self.switch_mode(mode_idx);
                } else if key == canon::cc('T', 'B') {
                    self.handle_tab_focus();
                }
            }

            if down {
                self.handle_scroll_key(key);
            }
        }
    }

    fn handle_tab_focus(&mut self) {
        let Some(active_window_id) = self.active_window else {
            return;
        };

        let mut focusable = Vec::new();
        self.widget_manager
            .collect_focusable_widgets(active_window_id, &mut focusable);

        if focusable.is_empty() {
            return;
        }

        let current_index = self
            .widget_manager
            .active_widget
            .and_then(|id| focusable.iter().position(|x| *x == id));

        let next_index = if let Some(idx) = current_index {
            if self.shift_down {
                if idx == 0 {
                    focusable.len() - 1
                } else {
                    idx - 1
                }
            } else {
                (idx + 1) % focusable.len()
            }
        } else {
            0
        };

        let old_widget = self.widget_manager.active_widget;
        let new_widget = focusable[next_index];
        self.widget_manager.active_widget = Some(new_widget);
        self.fb_dirty = true;

        let focused_sym = canon::canon(b'F', b'C', b'S');

        if let Some(old_id) = old_widget {
            if old_id != new_widget {
                let mut updates = graph::map();
                updates.insert(focused_sym, Value::Bool(false));
                graph::fiat(Some(old_id), canon::WIDGET, updates);
            }
        }

        let mut updates = graph::map();
        updates.insert(focused_sym, Value::Bool(true));
        graph::fiat(Some(new_widget), canon::WIDGET, updates);
    }

    fn tile_windows(&mut self) {
        let visible_windows = self.visible_window_ids();

        if visible_windows.is_empty() {
            return;
        }

        let count = visible_windows.len() as i32;
        let mut cols = 1;
        while cols * cols < count {
            cols += 1;
        }
        let rows = (count + cols - 1) / cols;

        let geo = self.fb_device.geometry();
        let screen_w = geo.width as i32;
        let screen_h = geo.height as i32;

        let w = screen_w / cols;
        let h = screen_h / rows;

        for (i, win_id) in visible_windows.iter().enumerate() {
            let row = (i as i32) / cols;
            let col = (i as i32) % cols;

            let x = col * w;
            let y = row * h;

            if let Some(entry) = self.windows.get_mut(win_id) {
                entry.window.x = x.max(0) as u64;
                entry.window.y = y.max(0) as u64;
                entry.window.width = w.max(MIN_WINDOW_WIDTH) as u64;
                entry.window.height = h.max(MIN_WINDOW_HEIGHT) as u64;
            }

            let mut props = BTreeMap::new();
            props.insert(canon::X, Value::U64(x.max(0) as u64));
            props.insert(canon::Y, Value::U64(y.max(0) as u64));
            props.insert(canon::WIDTH, Value::U64(w.max(MIN_WINDOW_WIDTH) as u64));
            props.insert(canon::HEIGHT, Value::U64(h.max(MIN_WINDOW_HEIGHT) as u64));

            self.update_window_props(*win_id, props);
        }
    }

    fn maybe_auto_tile_windows(&mut self) {
        if self.auto_layout_done {
            return;
        }

        let visible_windows = self.visible_window_ids();
        if visible_windows.len() < AUTO_TILE_MIN_WINDOWS {
            return;
        }

        let geo = self.fb_device.geometry();
        // Reserve banner space so tiled windows start below the toolbar.
        let available_height = (geo.height as i32).saturating_sub(AUTO_TILE_TOP_OFFSET);
        if available_height <= 0 {
            return;
        }

        let area = Rect::new(0, AUTO_TILE_TOP_OFFSET, geo.width, available_height as u32);
        self.layout_windows_in_area(area, &visible_windows);

        self.auto_layout_done = true;
        self.fb_dirty = true;
    }

    fn layout_windows_in_area(&mut self, area: Rect, window_ids: &[Uuid]) {
        if window_ids.is_empty() || area.width == 0 || area.height == 0 {
            return;
        }

        let count = window_ids.len();
        let mut cols = 1;
        while cols * cols < count {
            cols += 1;
        }
        let rows = (count + cols - 1) / cols;

        let items: Vec<LayoutItem> = window_ids
            .iter()
            .map(|id| LayoutItem {
                id: *id,
                min_width: MIN_WINDOW_WIDTH as u32,
                min_height: MIN_WINDOW_HEIGHT as u32,
                ..Default::default()
            })
            .collect();

        let spec = LayoutSpec::Grid { rows, cols };
        let rects = layout::layout(area, spec, &items, AUTO_TILE_MARGIN);

        for (win_id, rect) in rects {
            if let Some(entry) = self.windows.get_mut(&win_id) {
                entry.window.x = rect.x.max(0) as u64;
                entry.window.y = rect.y.max(0) as u64;
                entry.window.width = (rect.width as u64).max(MIN_WINDOW_WIDTH as u64);
                entry.window.height = (rect.height as u64).max(MIN_WINDOW_HEIGHT as u64);
            }

            let mut props = BTreeMap::new();
            props.insert(canon::X, Value::U64(rect.x.max(0) as u64));
            props.insert(canon::Y, Value::U64(rect.y.max(0) as u64));
            props.insert(
                canon::WIDTH,
                Value::U64((rect.width as u64).max(MIN_WINDOW_WIDTH as u64)),
            );
            props.insert(
                canon::HEIGHT,
                Value::U64((rect.height as u64).max(MIN_WINDOW_HEIGHT as u64)),
            );
            self.update_window_props(win_id, props);
        }
    }

    fn continue_widget_interaction(&mut self) {
        if let Some(widget_id) = self.widget_manager.active_widget {
            if let Some(widget) = self.widget_manager.widgets.get(&widget_id) {
                let wx = widget.x.unwrap_or(0) as i32;
                let wy = widget.y.unwrap_or(0) as i32;
                let local_x = self.cursor.x - wx;
                let local_y = self.cursor.y - wy;

                let mut updates = graph::map();
                updates.insert(canon::MOUSE_X, Value::I64(local_x as i64));
                updates.insert(canon::MOUSE_Y, Value::I64(local_y as i64));
                graph::fiat(Some(widget_id), canon::WIDGET, updates);
            }
        }
    }

    fn dispatch_action(&mut self, action: &str, window_id: Option<Uuid>) {
        if action == "ACTION.CLOSE_WINDOW" {
            if let Some(id) = window_id {
                 // Logic from on_close_button_up
                let mut props = BTreeMap::new();
                props.insert(canon::VISIBLE, Value::Bool(false));
                self.update_window_props(id, props);
            }
        }
    }

    fn end_widget_interaction(&mut self) {
        if let Some(widget_id) = self.widget_manager.active_widget {
            let mut updates = graph::map();
            updates.insert(canon::MOUSE_DOWN, Value::Bool(false));
            graph::fiat(Some(widget_id), canon::WIDGET, updates);

            // Check for valid click (release inside widget)
            if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
                 if let Some(surface) = self.windows.get(&win_id) {
                     let w = surface.window.width as i32;
                     let h = surface.window.height as i32;
                     if let Some(layout) = compute_window_layout(win_x, win_y, w, h, self.layout.title_bar_height as i32) {
                         let metrics = ContentMetrics::new(surface, &layout);
                         if let Some(hit_id) = self.widget_manager.hit_test_widgets(
                            win_id,
                            &layout,
                            &metrics,
                            self.cursor.x,
                            self.cursor.y
                         ) {
                             if hit_id == widget_id {
                                 // Valid Click!
                                 if let Some(widget) = self.widget_manager.widgets.get(&widget_id).cloned() {
                                     if let Some(action) = widget.action {
                                         self.dispatch_action(&action, Some(win_id));
                                     }
                                 }
                             }
                         }
                     }
                 }
            }

            self.widget_manager.active_widget = None;
        }
    }

    fn on_pointer_down(&mut self) {
        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            self.set_active_window(Some(win_id));

            let (win_width, win_height) = {
                let Some(surface) = self.windows.get(&win_id) else {
                    return;
                };
                (surface.window.width as i32, surface.window.height as i32)
            };
            let Some(layout) = compute_window_layout(win_x, win_y, win_width, win_height, self.layout.title_bar_height as i32) else {
                return;
            };



            let metrics = {
                let Some(surface) = self.windows.get(&win_id) else {
                    return;
                };
                ContentMetrics::new(surface, &layout)
            };

            if let Some(widget_id) = self.widget_manager.hit_test_widgets(
                win_id,
                &layout,
                &metrics,
                self.cursor.x,
                self.cursor.y,
            ) {
                self.widget_manager.active_widget = Some(widget_id);
                if let Some(widget) = self.widget_manager.widgets.get(&widget_id) {
                    let wx = widget.x.unwrap_or(0) as i32;
                    let wy = widget.y.unwrap_or(0) as i32;
                    let local_x = self.cursor.x - wx;
                    let local_y = self.cursor.y - wy;

                    let mut updates = graph::map();
                    updates.insert(canon::MOUSE_X, Value::I64(local_x as i64));
                    updates.insert(canon::MOUSE_Y, Value::I64(local_y as i64));
                    updates.insert(canon::MOUSE_DOWN, Value::Bool(true));
                    graph::fiat(Some(widget_id), canon::WIDGET, updates);

                    if widget.role == ROLE_TOOLBAR_BUTTON {
                        println!("Toolbar button clicked: {}", widget_id);
                        if let Some(action) = &widget.action {
                            println!("Action: {}", action);
                        }
                        return;
                    }
                }
                return;
            }

            if metrics.max_scroll > 0 {
                if let (Some(track), Some(thumb), Some(offset)) = (
                    metrics.scrollbar_track_rect,
                    metrics.scrollbar_thumb_rect,
                    metrics.scrollbar_thumb_offset,
                ) {
                    if rect_contains(&track, self.cursor.x, self.cursor.y) {
                        if rect_contains(&thumb, self.cursor.x, self.cursor.y) {
                            self.drag_state = Some(DragState {
                                window_id: win_id,
                                kind: DragKind::ScrollThumb {
                                    track_height: track.height as i32,
                                    thumb_height: thumb.height as i32,
                                    thumb_offset: offset,
                                    max_scroll: metrics.max_scroll,
                                    start_cursor_y: self.cursor.y,
                                },
                            });
                        } else {
                            let page = metrics.viewport_height.max(SCROLL_STEP_LINE);
                            if self.cursor.y < thumb.y {
                                self.scroll_window_by(win_id, -page);
                            } else {
                                self.scroll_window_by(win_id, page);
                            }
                        }
                        return;
                    }
                }
            }

            if let Some(edges) = hit_test_resize(
                win_x,
                win_y,
                win_width,
                win_height,
                self.cursor.x,
                self.cursor.y,
            ) {
                self.drag_state = Some(DragState {
                    window_id: win_id,
                    kind: DragKind::Resize {
                        edges,
                        start_cursor_x: self.cursor.x,
                        start_cursor_y: self.cursor.y,
                        start_x: win_x,
                        start_y: win_y,
                        start_w: win_width,
                        start_h: win_height,
                    },
                });
                return;
            }

            if self.cursor.y >= layout.title_y && self.cursor.y < layout.title_y + layout.title_h {
                self.drag_state = Some(DragState {
                    window_id: win_id,
                    kind: DragKind::Move {
                        offset_x: self.cursor.x - win_x,
                        offset_y: self.cursor.y - win_y,
                    },
                });
            }
        } else {
            self.set_active_window(None);
        }
    }

    fn continue_drag(&mut self) {
        let Some(drag) = &self.drag_state else {
            return;
        };

        match &drag.kind {
            DragKind::Move { offset_x, offset_y } => {
                let new_x = max(0, self.cursor.x - offset_x);
                let new_y = max(0, self.cursor.y - offset_y);

                if let Some(entry) = self.windows.get_mut(&drag.window_id) {
                    entry.window.x = new_x as u64;
                    entry.window.y = new_y as u64;
                }

                let mut props = BTreeMap::new();
                props.insert(canon::X, Value::U64(new_x as u64));
                props.insert(canon::Y, Value::U64(new_y as u64));

                self.update_window_props(drag.window_id, props);
            }
            DragKind::Resize {
                edges,
                start_cursor_x,
                start_cursor_y,
                start_x,
                start_y,
                start_w,
                start_h,
            } => {
                let dx = self.cursor.x - start_cursor_x;
                let dy = self.cursor.y - start_cursor_y;

                let mut new_x = *start_x;
                let mut new_y = *start_y;
                let mut new_w = *start_w;
                let mut new_h = *start_h;

                if edges.left {
                    let proposed_w = start_w - dx;
                    let clamped_w = max(MIN_WINDOW_WIDTH, proposed_w);
                    let delta = start_w - clamped_w;
                    new_x = start_x + delta;
                    new_w = clamped_w;
                } else if edges.right {
                    new_w = max(MIN_WINDOW_WIDTH, start_w + dx);
                }

                if edges.top {
                    let proposed_h = start_h - dy;
                    let clamped_h = max(MIN_WINDOW_HEIGHT, proposed_h);
                    let delta = start_h - clamped_h;
                    new_y = start_y + delta;
                    new_h = clamped_h;
                } else if edges.bottom {
                    new_h = max(MIN_WINDOW_HEIGHT, start_h + dy);
                }

                if new_x < 0 {
                    let overshoot = -new_x;
                    new_x = 0;
                    new_w = max(new_w + overshoot, MIN_WINDOW_WIDTH);
                }
                if new_y < 0 {
                    let overshoot = -new_y;
                    new_y = 0;
                    new_h = max(new_h + overshoot, MIN_WINDOW_HEIGHT);
                }

                if let Some(entry) = self.windows.get_mut(&drag.window_id) {
                    entry.window.x = new_x as u64;
                    entry.window.y = new_y as u64;
                    entry.window.width = new_w as u64;
                    entry.window.height = new_h as u64;
                }

                let mut props = BTreeMap::new();
                props.insert(canon::X, Value::U64(new_x as u64));
                props.insert(canon::Y, Value::U64(new_y as u64));
                props.insert(canon::WIDTH, Value::U64(new_w as u64));
                props.insert(canon::HEIGHT, Value::U64(new_h as u64));

                self.update_window_props(drag.window_id, props);
                self.clamp_scroll_for(drag.window_id);
            }
            DragKind::ScrollThumb {
                track_height,
                thumb_height,
                thumb_offset,
                max_scroll,
                start_cursor_y,
            } => {
                if *max_scroll <= 0 {
                    return;
                }
                let travel = (*track_height - *thumb_height).max(1);
                if travel <= 0 {
                    return;
                }
                let delta_pixels = self.cursor.y - start_cursor_y;
                let new_thumb_offset = clamp_i32(thumb_offset + delta_pixels, 0, travel);
                let ratio = new_thumb_offset as f32 / travel as f32;
                let new_scroll = ((ratio * *max_scroll as f32) + 0.5) as i32;
                self.set_scroll_offset(drag.window_id, new_scroll);
            }
            DragKind::CloseButton => {}
        }
    }

    fn max_window_z(&self) -> i64 {
        self.windows.values().map(|w| w.window.z).max().unwrap_or(0)
    }

    fn ensure_window_has_unique_z(&mut self, window_id: Uuid, prev_max_z: i64) {
        let Some(entry) = self.windows.get_mut(&window_id) else {
            return;
        };
        if entry.window.z > prev_max_z {
            return;
        }

        let new_z = prev_max_z.saturating_add(1);
        if entry.window.z == new_z {
            return;
        }

        entry.window.z = new_z;
        let mut props = BTreeMap::new();
        props.insert(canon::Z, Value::I64(new_z));
        self.update_window_props(window_id, props);
    }

    fn update_window_props(&self, window_id: Uuid, props: BTreeMap<canon::Symbol, Value>) {
        if props.is_empty() {
            return;
        }
        let req = AbiRequest::PropsSet {
            request: GraphPropsRequest {
                node: window_id,
                props,
            },
        };
        let _ = userland::runtime().call(req);
    }

    fn set_active_window(&mut self, window_id: Option<Uuid>) {
        if self.active_window == window_id {
            return;
        }

        let prev_window = self.active_window;

        if let Some(prev) = self.active_window.take() {
            if let Some(entry) = self.windows.get_mut(&prev) {
                entry.window.active = false;
            }
            let mut props = BTreeMap::new();
            props.insert(canon::ACTIVE, Value::Bool(false));
            self.update_window_props(prev, props);
        }

        if let Some(id) = window_id {
            let new_z = self.max_window_z().saturating_add(1);
            if let Some(entry) = self.windows.get_mut(&id) {
                entry.window.active = true;
                entry.window.z = new_z;
            }

            let mut props = BTreeMap::new();
            props.insert(canon::ACTIVE, Value::Bool(true));
            props.insert(canon::Z, Value::I64(new_z));
            self.update_window_props(id, props);
            self.active_window = Some(id);
            self.bump_window(id);
        } else {
            self.active_window = None;
            self.update_graph_state();
        }

        self.content_dirty = true;
    }

    fn ensure_active_window(&mut self) {
        if let Some(active) = self.active_window {
            if let Some(surface) = self.windows.get(&active) {
                if surface.window.visible {
                    return;
                }
            }
        }

        if let Some((id, _)) = self
            .windows
            .iter()
            .filter(|(_, surface)| surface.window.active && surface.window.visible)
            .max_by_key(|(_, surface)| surface.window.z)
        {
            if self.active_window != Some(*id) {
                self.active_window = Some(*id);
                self.content_dirty = true;
            }
            return;
        }

        if let Some(id) = self.ordered_window_ids().into_iter().last() {
            self.set_active_window(Some(id));
        } else {
            self.set_active_window(None);
        }
    }

    fn cursor_kind_for_edges(edges: &ResizeEdges) -> CursorKind {
        match (edges.left, edges.right, edges.top, edges.bottom) {
            (true, false, true, false) => CursorKind::ResizeNW,
            (false, true, true, false) => CursorKind::ResizeNE,
            (true, false, false, true) => CursorKind::ResizeSW,
            (false, true, false, true) => CursorKind::ResizeSE,
            (true, false, false, false) => CursorKind::ResizeW,
            (false, true, false, false) => CursorKind::ResizeE,
            (false, false, true, false) => CursorKind::ResizeN,
            (false, false, false, true) => CursorKind::ResizeS,
            _ => CursorKind::Arrow,
        }
    }

    fn compute_cursor_kind(&self) -> CursorKind {
        if let Some(drag) = &self.drag_state {
            return match &drag.kind {
                DragKind::Move { .. } => CursorKind::Move,
                DragKind::Resize { edges, .. } => Self::cursor_kind_for_edges(edges),
                DragKind::ScrollThumb { .. } => CursorKind::Move,
                DragKind::CloseButton => CursorKind::Arrow,
            };
        }

        if let Some((win_id, win_x, win_y)) = self.find_window_at(self.cursor.x, self.cursor.y) {
            let Some(surface) = self.windows.get(&win_id) else {
                return CursorKind::Arrow;
            };

            let win_width = surface.window.width as i32;
            let win_height = surface.window.height as i32;

            if let Some(edges) = hit_test_resize(
                win_x,
                win_y,
                win_width,
                win_height,
                self.cursor.x,
                self.cursor.y,
            ) {
                return Self::cursor_kind_for_edges(&edges);
            }
        }

        CursorKind::Arrow
    }

    fn update_cursor_kind(&mut self) {
        let kind = self.compute_cursor_kind();
        self.cursor.set_kind(kind);
    }

    fn ingest_cursor(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::CURSOR {
            return;
        }
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_i64());
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_i64());
        let visible = thing.fields.get(&canon::VISIBLE).and_then(|v| v.as_bool());
        let geo = self.fb_device.geometry();
        let (width, height) = (geo.width as usize, geo.height as usize);
        self.cursor.set_from_graph(x, y, visible, width, height);
    }

    fn ingest_widget(&mut self, thing: &userland::GraphThing) {
        self.widget_manager.ingest_widget(thing, &mut self.windows);
    }

    fn ingest_wallpaper(&mut self, thing: &userland::GraphThing) {
        if thing.kind != canon::WALLPAPER {
            return;
        }

        let mode_node = thing.fields.get(&canon::MODE).and_then(|v| v.as_uuid());
        let place_id = thing.fields.get(&canon::PLACE).and_then(|v| v.as_uuid());

        let layers = self
            .wallpapers
            .get(&thing.id)
            .map(|w| w.layers.clone())
            .unwrap_or_else(Vec::new);

        let wallpaper = WallpaperState {
            id: thing.id,
            mode_node,
            place_id,
            layers,
        };
        self.wallpapers.insert(thing.id, wallpaper);
    }

    fn ingest_layer(&mut self, thing: &userland::GraphThing) {
        if let Some(Value::Uuid(wallpaper_id)) = thing.fields.get(&canon::WALLPAPER) {
            self.wallpapers
                .entry(*wallpaper_id)
                .or_insert_with(|| WallpaperState {
                    id: *wallpaper_id,
                    mode_node: None,
                    place_id: None,
                    layers: Vec::new(),
                });

            let index = thing
                .fields
                .get(&canon::INDEX)
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as usize;
            let kind_sym = thing
                .fields
                .get(&canon::TYPE)
                .and_then(|v| v.as_symbol())
                .unwrap_or(canon::SOLID_COLOR);

            let kind = if kind_sym == canon::SOLID_COLOR {
                let color_u64 = thing
                    .fields
                    .get(&canon::COLOR)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0xFF000000);
                let r = ((color_u64 >> 16) & 0xFF) as u8;
                let g = ((color_u64 >> 8) & 0xFF) as u8;
                let b = (color_u64 & 0xFF) as u8;
                LayerKind::SolidColor(Rgba::opaque(r, g, b))
            } else if kind_sym == canon::IMAGE {
                let filename = thing
                    .fields
                    .get(&canon::IMAGE)
                    .and_then(|v| v.as_text())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                LayerKind::Image(filename)
            } else {
                LayerKind::SolidColor(Rgba::opaque(0, 0, 0))
            };

            let scroll_x = thing
                .fields
                .get(&canon::SCROLL_X)
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as f64
                / 1000.0;
            let scroll_y = thing
                .fields
                .get(&canon::SCROLL_Y)
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as f64
                / 1000.0;

            let layer = LayerState {
                id: thing.id,
                kind,
                scroll_factor_x: scroll_x,
                scroll_factor_y: scroll_y,
                z_index: index,
            };

            self.layers.insert(thing.id, layer.clone());

            // Update the wallpaper's layer list
            if let Some(wallpaper) = self.wallpapers.get_mut(wallpaper_id) {
                if !wallpaper.layers.contains(&thing.id) {
                    wallpaper.layers.push(thing.id);
                }
                wallpaper.layers.sort_by(|a, b| {
                    let layer_a = self.layers.get(a).map(|l| l.z_index).unwrap_or(0);
                    let layer_b = self.layers.get(b).map(|l| l.z_index).unwrap_or(0);
                    layer_a.cmp(&layer_b)
                });
            }
        }
    }

    fn ensure_window_chrome(&mut self, window_id: Uuid, title: &str) -> (Uuid, Uuid) {
        let title_bar_id = userland::simple_uuid(alloc::format!("TitleBar:{}", window_id).as_bytes());
        let title_text_id = userland::simple_uuid(alloc::format!("TitleText:{}", window_id).as_bytes());
        let close_btn_id = userland::simple_uuid(alloc::format!("CloseBtn:{}", window_id).as_bytes());

        // 1. Title Bar Container
        let mut fields = userland::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        // Use container.vertical but force row direction to fake a horizontal container
        fields.insert(canon::ROLE, Value::Text("container.vertical".into())); 
        fields.insert(canon::cc('F', 'D'), Value::Text("row".into())); // FlexDirection::Row
        fields.insert(canon::cc('A', 'I'), Value::Text("center".into())); // AlignItems::Center
        fields.insert(canon::PARENT, Value::Uuid(window_id));
        fields.insert(canon::HEIGHT, Value::U64(self.layout.title_bar_height as u64));
        fields.insert(canon::WIDTH, Value::Text("100%".into()));
        fields.insert(canon::GAP, Value::I64(4));
        userland::fiat(Some(title_bar_id), canon::WIDGET, fields);

        // 2. Title Text Label
        let mut fields = userland::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        fields.insert(canon::ROLE, Value::Text("label".into()));
        fields.insert(canon::PARENT, Value::Uuid(title_bar_id));
        fields.insert(canon::LABEL, Value::Text(title.into()));
        fields.insert(canon::cc('F', 'G'), Value::I64(1)); 
        userland::fiat(Some(title_text_id), canon::WIDGET, fields);

        // 3. Close Button
        let mut fields = userland::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        fields.insert(canon::ROLE, Value::Text("button".into()));
        fields.insert(canon::PARENT, Value::Uuid(title_bar_id));
        fields.insert(canon::LABEL, Value::Text("".into()));
        fields.insert(canon::ICON_NAME, Value::Text("close".into()));
        fields.insert(canon::ACTION, Value::Text("ACTION.CLOSE_WINDOW".into()));
        fields.insert(canon::WIDTH, Value::U64(CLOSE_BUTTON_SIZE as u64));
        fields.insert(canon::HEIGHT, Value::U64(CLOSE_BUTTON_SIZE as u64));
        userland::fiat(Some(close_btn_id), canon::WIDGET, fields);

        (title_bar_id, close_btn_id)
    }

    fn ingest_window(&mut self, mut window: Window) {
        window.width = window.width.max(MIN_WINDOW_WIDTH as u64);
        window.height = window.height.max(MIN_WINDOW_HEIGHT as u64);

        let window_id = window.id;
        let is_new = !self.windows.contains_key(&window_id);
        let prev_max_z = if is_new {
            Some(self.max_window_z())
        } else {
            None
        };

        // Create/Update Chrome Widgets
        let (title_bar_id, close_btn_id) = self.ensure_window_chrome(window_id, &window.title);

        if let Some(entry) = self.windows.get_mut(&window_id) {
            entry.window = window;
        } else {
            let btn_state = widget_button::State {
                label: "".to_string(),
                target: "close_window".to_string(),
                pressed: false,
                hovered: false,
                focused: false,
                icon_name: Some("close".to_string()),
                show_label: true,
                bind_node: None,
                bind_index: None,
            };

            self.windows.insert(
                window_id,
                WindowSurface {
                    window,
                    surface_id: None,
                    text: String::new(),
                    bitmap: None,
                    scroll_y: 0,
                    scrollbar_widget_id: None,
                    caret: Caret::default(),
                    title_bar_id: Some(title_bar_id),
                    close_button_id: close_btn_id,
                    close_button_state: btn_state,
                    close_button_bitmap: None,
                    repeat: false,
                },
            );
        }

        if is_new {
            let window = &self.windows[&window_id].window;

            // Check for Place association
            let place_id = window.place_id;
            let is_place_root = window.is_place_root;

            // Fallback to mode_index for backward compatibility
            let target_mode = window
                .mode_index
                .map(|i| i as usize)
                .unwrap_or(self.active_mode);

            let mut assigned = false;

            if let Some(place_id) = place_id {
                // Find mode for this place
                let mut found_mode = None;
                for (idx, mode) in self.modes.iter().enumerate() {
                    if mode.place_id == Some(place_id) {
                        found_mode = Some(idx);
                        break;
                    }
                }

                if let Some(idx) = found_mode {
                    if is_place_root {
                        self.modes[idx].root_window = Some(window_id);
                    } else {
                        self.modes[idx].windows.push(window_id);
                    }
                    assigned = true;
                }
            }

            if !assigned {
                if target_mode < self.modes.len() {
                    if window.is_root {
                        self.modes[target_mode].root_window = Some(window_id);
                    } else {
                        self.modes[target_mode].windows.push(window_id);
                    }
                }
            }
        }

        if let Some(prev_max_z) = prev_max_z {
            self.ensure_window_has_unique_z(window_id, prev_max_z);
        }
        if let Some(target) = self.windows.get(&window_id).and_then(|w| w.window.target) {
            if let Some(entry) = self.windows.get_mut(&window_id) {
                entry.surface_id = Some(target);
            }
        }
        let is_active = self
            .windows
            .get(&window_id)
            .map(|w| w.window.active)
            .unwrap_or(false);
        if is_active {
            self.set_active_window(Some(window_id));
        } else if self.active_window == Some(window_id) {
            self.set_active_window(None);
        } else if is_new {
            self.bump_window(window_id);
        }

        self.clamp_scroll_for(window_id);
        self.maybe_auto_tile_windows();
    }

    fn bump_window(&mut self, window_id: Uuid) {
        // Find which mode contains this window and bump it
        for mode in &mut self.modes {
            if let Some(pos) = mode.windows.iter().position(|w| *w == window_id) {
                mode.windows.remove(pos);
                mode.windows.push(window_id);
                break;
            }
        }
        self.update_graph_state();
    }

    fn active_wallpaper(&self) -> Option<&WallpaperState> {
        let place_wallpaper = self
            .modes
            .get(self.active_mode)
            .and_then(|mode| mode.place_id)
            .and_then(|place_id| {
                self.wallpapers
                    .values()
                    .find(|wallpaper| wallpaper.place_id == Some(place_id))
            });

        if place_wallpaper.is_some() {
            return place_wallpaper;
        }

        let mode_node_id =
            userland::simple_uuid(alloc::format!("ModeF{}", self.active_mode + 1).as_bytes());

        self.wallpapers
            .values()
            .find(|wallpaper| wallpaper.mode_node == Some(mode_node_id))
    }

    fn draw_background(&self, scene: &mut Scene, width: usize, height: usize) {
        let screen_rect = Rect::new(0, 0, width as u32, height as u32);

        if let Some(wallpaper) = self.active_wallpaper() {
            build_wallpaper_scene(
                scene,
                screen_rect,
                wallpaper,
                &self.layers,
                &self.bitmaps,
                (self.frame_no as f64, self.frame_no as f64),
            );
        } else {
            scene.push(SceneItem::FillRect {
                rect: screen_rect,
                color: CLEAR_COLOR,
            });
        }
    }

    fn draw_windows(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        for id in self.ordered_window_ids() {
            if let Some(surface) = self.windows.get(&id).cloned() {
                if surface.window.is_root {
                    self.draw_window_frameless(scene, &surface, fb_width, fb_height);
                } else if self.debug_layout_mode {
                    self.draw_debug_window(scene, &surface, fb_width, fb_height);
                } else {
                    self.draw_window(scene, &surface, fb_width, fb_height);
                    if self.debug_overlay_mode {
                        self.draw_debug_window(scene, &surface, fb_width, fb_height);
                    }
                }
            }
        }
    }

    fn draw_debug_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);
        let is_active = surface.window.active || self.active_window == Some(surface.window.id);

        let border_color = if is_active {
            Rgba::new(0xFF, 0, 0, 0xFF) // Red for active
        } else {
            Rgba::new(0x00, 0, 0xFF, 0xFF) // Blue for inactive
        };

        // Draw bounding box (outline)
        // Top
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, 2),
            color: border_color,
        });
        // Bottom
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, (y + h - 2) as i32, w as u32, 2),
            color: border_color,
        });
        // Left
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, 2, h as u32),
            color: border_color,
        });
        // Right
        scene.push(SceneItem::FillRect {
            rect: Rect::new((x + w - 2) as i32, y as i32, 2, h as u32),
            color: border_color,
        });
        // Draw ID or Title in the center
        let label = alloc::format!("ID: {:?}\nActive: {}", surface.window.id, is_active);
        scene.push(SceneItem::DrawTextBlock {
            rect: Rect::new(x as i32 + 4, y as i32 + 4, (w - 8) as u32, (h - 8) as u32),
            text: label,
            color: border_color,
            scroll_offset: 0,
        });

        // Draw Widgets
        let client_x = x as i32;
        let client_y = y as i32;
        let client_w = w as i32;
        let client_h = h as i32;

        self.widget_manager.draw_debug_widgets(
            scene,
            surface.window.id,
            client_x,
            client_y,
            client_w,
            client_h,
            self.layout.title_bar_height as i32,
            surface.scroll_y,
        );
    }

    fn draw_max_mode(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        if let Some(active_id) = self.active_window {
            if let Some(surface) = self.windows.get(&active_id).cloned() {
                if surface.window.visible {
                    self.draw_window_frameless(scene, &surface, fb_width, fb_height);
                    return;
                }
            }
        }

        self.draw_background(scene, fb_width, fb_height);
    }

    fn draw_window_frameless(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = if surface.window.is_root {
            fb_width
        } else {
            surface.window.width as usize
        };
        let h = if surface.window.is_root {
            fb_height
        } else {
            surface.window.height as usize
        };
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
        });

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
            color: self.layout.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                image: bmp.clone(),
                repeat: surface.repeat,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });
        }

        let has_widgets = self
            .widget_manager
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));

        if has_widgets {
            let layout = WindowLayout {
                title_x: 0,
                title_y: 0,
                title_w: 0,
                title_h: 0,
                client_x: x as i32,
                client_y: y as i32,
                client_w: w as i32,
                client_h: h as i32,
            };
            self.widget_manager.draw_widgets(
                scene,
                surface.window.id,
                &layout,
                layout.client_w,
                self.windows.get(&surface.window.id),
                &self.windows,
                None,
            );
        }

        if !surface.window.active {
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x as i32, y as i32, w as u32, h as u32),
                color: self.layout.inactive_veil,
            });
        }

        scene.push(SceneItem::ClipPop);
    }

    fn draw_window(
        &self,
        scene: &mut Scene,
        surface: &WindowSurface,
        fb_width: usize,
        fb_height: usize,
    ) {
        let w = surface.window.width as usize;
        let h = surface.window.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        let x = min(surface.window.x as usize, fb_width);
        let y = min(surface.window.y as usize, fb_height);
        let is_active = surface.window.active || self.active_window == Some(surface.window.id);

        let title_color = if is_active {
            self.layout.title_active
        } else {
            self.layout.title_inactive
        };
        let title_text = if is_active {
            self.layout.title_text_active
        } else {
            self.layout.title_text_inactive
        };
        let frame_fill = if is_active {
            self.layout.title_active
        } else {
            self.layout.title_inactive
        };

        // --- Shadow ---
        // Feathered drop shadow (expanding layers)
        // Offset (4, 4)
        let sx = x as i32 + 4;
        let sy = y as i32 + 4;
        let sw = w as u32;
        let sh = h as u32;

        // Core
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx, sy, sw, sh),
            color: Rgba::new(0x40, 0, 0, 0),
        });
        // Feather 1
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 1, sy - 1, sw + 2, sh + 2),
            color: Rgba::new(0x20, 0, 0, 0),
        });
        // Feather 2
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 2, sy - 2, sw + 4, sh + 4),
            color: Rgba::new(0x10, 0, 0, 0),
        });
        // Feather 3
        scene.push(SceneItem::FillRect {
            rect: Rect::new(sx - 3, sy - 3, sw + 6, sh + 6),
            color: Rgba::new(0x08, 0, 0, 0),
        });

        let Some(layout) = compute_window_layout(x as i32, y as i32, w as i32, h as i32, self.layout.title_bar_height as i32) else {
            return;
        };
        let x0 = x as i32;
        let y0 = y as i32;
        let w_i = w as i32;
        let h_i = h as i32;
        let inner_x0 = x0 + BORDER_OUTER_THICKNESS;
        let inner_y0 = y0 + BORDER_OUTER_THICKNESS;
        let inner_w = w_i - BORDER_OUTER_THICKNESS * 2;
        let inner_h = h_i - BORDER_OUTER_THICKNESS * 2;
        let content_x0 = inner_x0 + BORDER_3D_THICKNESS;
        let content_y0 = inner_y0 + BORDER_3D_THICKNESS;
        let content_w = inner_w - BORDER_3D_THICKNESS * 2;
        let content_h = inner_h - BORDER_3D_THICKNESS * 2;

        scene.push(SceneItem::ClipPush {
            rect: Rect::new(x0, y0, w as u32, h as u32),
        });

        // 1. Outer Border
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x0, y0, w as u32, h as u32),
            color: self.layout.frame_outer,
        });

        // 2. Bevel lines to make the frame pop
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0,
                inner_w as u32,
                BORDER_3D_THICKNESS as u32,
            ),
            color: self.layout.frame_hilight,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0,
                BORDER_3D_THICKNESS as u32,
                inner_h as u32,
            ),
            color: self.layout.frame_hilight,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0 + inner_w - BORDER_3D_THICKNESS,
                inner_y0,
                BORDER_3D_THICKNESS as u32,
                inner_h as u32,
            ),
            color: self.layout.frame_shadow,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                inner_x0,
                inner_y0 + inner_h - BORDER_3D_THICKNESS,
                inner_w as u32,
                BORDER_3D_THICKNESS as u32,
            ),
            color: self.layout.frame_shadow,
        });

        // 3. Inner Frame (Background / focus ring)
        scene.push(SceneItem::FillRect {
            rect: Rect::new(content_x0, content_y0, content_w as u32, content_h as u32),
            color: frame_fill,
        });

        // 4. Titlebar Background (Mechanical fallback)
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                layout.title_x,
                layout.title_y,
                layout.title_w as u32,
                layout.title_h as u32,
            ),
            color: title_color,
        });

        // Bottom line of titlebar
        scene.push(SceneItem::FillRect {
            rect: Rect::new(
                layout.title_x,
                layout.title_y + layout.title_h - 1,
                layout.title_w as u32,
                1,
            ),
            color: self.layout.frame_shadow,
        });

        // 5. Semantic TitleBar Widget
        if let Some(title_bar_id) = surface.title_bar_id {
            self.widget_manager.draw_specific_widget(
                scene,
                surface.window.id,
                title_bar_id,
                layout.title_x,
                layout.title_y,
                layout.title_w,
                layout.title_h,
                &self.windows,
            );
        }

        // 6. Client Area
        let client_y = layout.client_y + 1;
        let client_h = (layout.client_h - 1).max(0);
        let client_rect = Rect::new(
            layout.client_x,
            client_y,
            layout.client_w as u32,
            client_h as u32,
        );

        // --- Content / Widgets ---

        let metrics = ContentMetrics::new(surface, &layout);
        let widget_area_width = layout.client_w; // simplified?

        let has_widgets = self
            .widget_manager
            .widgets
            .values()
            .any(|w| w.parent == Some(surface.window.id));

        if has_widgets {
            // Exclude title bar from client area drawing
            self.widget_manager.draw_widgets(
                scene,
                surface.window.id,
                &layout,
                widget_area_width,
                self.windows.get(&surface.window.id),
                &self.windows,
                surface.title_bar_id,
            );
        } else {
            let content_rect = metrics.content_rect;

            if content_rect.width > 0 && content_rect.height > 0 {
                if let Some(bmp) = &surface.bitmap {
                    scene.push(SceneItem::BlitImage {
                        rect: content_rect,
                        image: bmp.clone(),
                        repeat: false,
                        offset: (0, 0),
                    });
                } else if !surface.text.is_empty() {
                    scene.push(SceneItem::DrawTextBlock {
                        rect: content_rect,
                        text: surface.text.clone(),
                        color: COLOR_TEXT,
                        scroll_offset: surface.scroll_y,
                    });

                    // Draw caret
                    if is_active && surface.caret.visible {
                        let cx = content_rect.x + surface.caret.x;
                        let cy = content_rect.y + surface.caret.y - surface.scroll_y;
                        if cy + surface.caret.height >= content_rect.y
                            && cy < content_rect.y + content_rect.height as i32
                        {
                            scene.push(SceneItem::FillRect {
                                rect: Rect::new(
                                    cx,
                                    cy,
                                    surface.caret.width as u32,
                                    surface.caret.height as u32,
                                ),
                                color: COLOR_TEXT,
                            });
                        }
                    }
                }
            }
        }

        self.draw_scrollbar_overlay(scene, surface.window.id, &metrics);

        scene.push(SceneItem::ClipPop);
    }

    fn draw_scrollbar_overlay(&self, scene: &mut Scene, window_id: Uuid, metrics: &ContentMetrics) {
        let track_rect = match metrics.scrollbar_track_rect {
            Some(rect) => rect,
            None => return,
        };

        scene.push(SceneItem::FillRect {
            rect: track_rect,
            color: SCROLLBAR_TRACK_COLOR,
        });

        let thumb_rect = match metrics.scrollbar_thumb_rect {
            Some(rect) => rect,
            None => return,
        };

        let dragging_thumb = matches!(
            &self.drag_state,
            Some(DragState {
                kind: DragKind::ScrollThumb { .. },
                window_id: drag_window,
            }) if *drag_window == window_id
        );

        let thumb_color = if dragging_thumb {
            SCROLLBAR_THUMB_HILIGHT
        } else {
            SCROLLBAR_THUMB_COLOR
        };

        scene.push(SceneItem::FillRect {
            rect: thumb_rect,
            color: thumb_color,
        });

        if thumb_rect.height > 1 {
            scene.push(SceneItem::FillRect {
                rect: Rect::new(thumb_rect.x, thumb_rect.y, thumb_rect.width, 1),
                color: SCROLLBAR_THUMB_HILIGHT,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(
                    thumb_rect.x,
                    thumb_rect.y + thumb_rect.height as i32 - 1,
                    thumb_rect.width,
                    1,
                ),
                color: SCROLLBAR_THUMB_SHADOW,
            });
        }
    }

    fn draw_cursor(&self, scene: &mut Scene, fb_width: usize, fb_height: usize) {
        if !self.cursor.visible {
            return;
        }
        let base_x = clamp_i32(self.cursor.x, 0, fb_width.saturating_sub(1) as i32) as i32;
        let base_y = clamp_i32(self.cursor.y, 0, fb_height.saturating_sub(1) as i32) as i32;
        let icon = self.cursor_sprites.for_kind(self.cursor.kind);

        scene.push(SceneItem::DrawCursor {
            origin: (base_x, base_y),
            sprite: icon.bitmap.clone(),
            hotspot: icon.hotspot,
        });
    }

    pub fn set_cursor(&mut self, x: i32, y: i32, buttons: u8) {
        self.cursor.x = x;
        self.cursor.y = y;
        self.cursor.buttons = buttons;
    }

    fn ensure_mode_app(&mut self, mode_idx: usize) {
        let mode = &mut self.modes[mode_idx];
        if mode.root_window.is_some() {
            return;
        }

        if let Some(place_id) = mode.place_id {
            if let Some(place_thing) = userland::graph::get_thing(place_id) {
                if let Some(app_name) = place_thing
                    .fields
                    .get(&canon::APP)
                    .and_then(|v| v.as_text())
                {
                    self.launch_app_for_place(app_name, place_id, mode_idx);
                }
            }
        }
    }

    fn launch_app_for_place(&self, app_name: &str, place_id: Uuid, mode_idx: usize) {
        let intent_id = next_uuid();
        let mut fields = userland::map();
        fields.insert(canon::KIND, Value::Symbol(canon::LAUNCH_INTENT));
        fields.insert(canon::APP, Value::Text(app_name.to_string()));
        fields.insert(canon::PLACE, Value::Uuid(place_id));
        fields.insert(canon::MODE_INDEX, Value::I64(mode_idx as i64));

        userland::fiat(Some(intent_id), canon::LAUNCH_INTENT, fields);

        userland::sys::spawn(app_name);
    }
}

fn default_window(id: Uuid) -> Window {
    Window {
        id,
        title: "window".to_string(),
        x: 32,
        y: 32,
        width: 320,
        height: 200,
        z: 0,
        visible: true,
        target: None,
        active: false,
        is_root: false,
        is_place_root: false,
        place_id: None,
        mode_index: None,
        window_rect: None,
        gap: None,
        flex_direction: None,
        justify_content: None,
        align_items: None,
        tile_mode: None,
    }
}

pub fn sanitize_fb_info(info: FramebufferGeometry) -> FramebufferGeometry {
    const MAX_DIM: u32 = 4096;
    let width = info.width.clamp(1, MAX_DIM);
    let height = info.height.clamp(1, MAX_DIM);
    let mut pitch = if info.pitch >= width * 4 && info.pitch <= width * 8 {
        info.pitch
    } else {
        width * 4
    };
    if pitch < width {
        pitch = width;
    }
    let bpp = if info.bpp == 24 || info.bpp == 32 {
        info.bpp
    } else {
        32
    };
    FramebufferGeometry {
        width,
        height,
        pitch,
        bpp,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FramebufferTarget {
    pub info: FramebufferGeometry,
    pub addr: *mut u32,
    pub len_bytes: usize,
}

#[cfg(feature = "kernel_standalone")]
pub trait RunnableApp {
    fn tick(&mut self, ctx: &mut userland::app::AppContext<'_>, tick: u64);
    fn on_event(&mut self, ctx: &mut userland::app::AppContext<'_>, ev: userland::AppEvent);
}

#[cfg(feature = "kernel_standalone")]
impl<T: userland::app::App> RunnableApp for T {
    fn tick(&mut self, ctx: &mut userland::app::AppContext<'_>, tick: u64) {
        self.tick(ctx, tick)
    }
    fn on_event(&mut self, ctx: &mut userland::app::AppContext<'_>, ev: userland::AppEvent) {
        self.on_event(ctx, ev)
    }
}

#[cfg(feature = "kernel_standalone")]
struct RunningAppInstance {
    app: alloc::boxed::Box<dyn RunnableApp>,
    state: userland::app::AppState,
    app_id: usize,
}

#[cfg(feature = "kernel_standalone")]
static PENDING_SPAWNS: spin::Mutex<Vec<String>> = spin::Mutex::new(Vec::new());

#[cfg(feature = "kernel_standalone")]
pub fn request_spawn(name: &str) {
    PENDING_SPAWNS.lock().push(name.to_string());
}
