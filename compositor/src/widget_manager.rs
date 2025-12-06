use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use uuid::Uuid;

use userland::ui_graph::Widget;
use userland::{canon, graph, Thingable, Value};

use crate::bitmap::{decode_bmp, Bitmap};
use crate::layout::{
    self, AlignItems, FlexDirection, FlexWrap, JustifyContent, LayoutItem, LayoutSpec,
};
use crate::scene::{Scene, SceneItem};
use crate::types::{
    Rect, Rgba, BORDER_THICKNESS, BTN_BORDER, BTN_FACE, BTN_GLYPH, COLOR_TEXT, FONT_HEIGHT,
    ROLE_CONTAINER_VERTICAL, ROLE_EDITOR_ROOT, ROLE_TOOLBAR, ROLE_TOOLBAR_BUTTON, THEME,
    TITLE_BAR_HEIGHT, TOOLBAR_BUTTON_SIZE, TOOLBAR_BUTTON_SPACING, TOOLBAR_HEIGHT,
};
use crate::window::{compute_window_layout, ContentMetrics, WindowLayout, WindowSurface};

pub struct WidgetManager {
    pub widgets: BTreeMap<Uuid, Widget>,
    pub active_widget: Option<Uuid>,
}

impl WidgetManager {
    pub fn new() -> Self {
        Self {
            widgets: BTreeMap::new(),
            active_widget: None,
        }
    }

    pub fn ingest_widget(
        &mut self,
        thing: &userland::GraphThing,
        windows: &mut BTreeMap<Uuid, WindowSurface>,
    ) {
        if let Some(existing) = self.widgets.get_mut(&thing.id) {
            existing.update(thing);

            if let Some(parent_id) = existing.parent {
                if let Some(scroll_y) = thing.fields.get(&canon::SCROLL_Y).and_then(|v| v.as_i64())
                {
                    if let Some(window) = windows.get_mut(&parent_id) {
                        if window.scrollbar_widget_id == Some(existing.id) {
                            window.scroll_y = scroll_y as i32;
                        }
                    }
                }
            }
        } else {
            if let Some(widget) = Widget::load(thing) {
                if let Some(parent_id) = widget.parent {
                    if let Some(scroll_y) =
                        thing.fields.get(&canon::SCROLL_Y).and_then(|v| v.as_i64())
                    {
                        if let Some(window) = windows.get_mut(&parent_id) {
                            if window.scrollbar_widget_id == Some(widget.id) {
                                window.scroll_y = scroll_y as i32;
                            }
                        }
                    }
                }
                self.widgets.insert(widget.id, widget);
            }
        }
    }

    pub fn draw_widgets(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        layout: &WindowLayout,
        widget_area_width: i32,
        window_surface: Option<&WindowSurface>,
        windows: &BTreeMap<Uuid, WindowSurface>,
    ) {
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(window_id))
            .map(|w| w.id)
            .collect();

        if root_widgets.is_empty() {
            return;
        }

        let mut relative_widgets: Vec<Uuid> = Vec::new();
        let mut overlay_widgets: Vec<Uuid> = Vec::new();

        for widget_id in &root_widgets {
            if let Some(widget) = self.widgets.get(widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    overlay_widgets.push(*widget_id);
                } else {
                    relative_widgets.push(*widget_id);
                }
            }
        }

        let x_offset = layout.client_x;
        let width = widget_area_width.max(0);
        let y_offset = layout.client_y;
        let height = layout.client_h.max(0);

        // Determine layout spec from window properties
        let (gap, spec) = if let Some(surface) = window_surface {
            let w = &surface.window;
            let gap = w.gap.unwrap_or(0);
            let (direction, wrap, justify, align) = if let Some(dir) = w.flex_direction {
                (
                    dir,
                    FlexWrap::NoWrap,
                    w.justify_content.unwrap_or_default(),
                    w.align_items.unwrap_or(AlignItems::Start),
                )
            } else {
                (
                    FlexDirection::Column,
                    FlexWrap::NoWrap,
                    JustifyContent::Start,
                    AlignItems::Stretch,
                )
            };
            let spec = LayoutSpec::Flex {
                direction,
                wrap,
                justify,
                align,
            };
            (gap, spec)
        } else {
            (
                0,
                LayoutSpec::Flex {
                    direction: FlexDirection::Column,
                    wrap: FlexWrap::NoWrap,
                    justify: JustifyContent::Start,
                    align: AlignItems::Stretch,
                },
            )
        };

        // Use layout engine for relative widgets
        if !relative_widgets.is_empty() {
            self.layout_and_draw_children(
                scene,
                window_id,
                Rect::new(
                    x_offset as i32,
                    y_offset as i32,
                    width as u32,
                    height as u32,
                ),
                &relative_widgets,
                spec,
                gap,
                windows,
            );
        }

        // Draw overlay widgets
        for widget_id in overlay_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                let wx = widget.x.unwrap_or(0) as i32;
                let wy = widget.y.unwrap_or(0) as i32;
                let ww = widget.width.unwrap_or(0) as i32;
                let wh = widget.height.unwrap_or(0) as i32;

                self.draw_widget_recursive(
                    scene,
                    window_id,
                    widget,
                    x_offset + wx,
                    y_offset + wy,
                    ww,
                    wh,
                    windows,
                );
            }
        }
    }

    fn layout_and_draw_children(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        container_rect: Rect,
        children_ids: &[Uuid],
        spec: LayoutSpec,
        gap: i32,
        windows: &BTreeMap<Uuid, WindowSurface>,
    ) {
        let items: Vec<LayoutItem> = children_ids
            .iter()
            .filter_map(|id| {
                self.widgets.get(id).map(|w| {
                    // Map widget sizing hints onto the flex layout item; width/height act as the preferred
                    // size, with a 30px fallback height so rows stay visible even without hints.
                    let min_width = w.min_width.unwrap_or(w.width.unwrap_or(0)) as u32;
                    let min_height = w.min_height.unwrap_or(w.height.unwrap_or(30)) as u32;
                    LayoutItem {
                        id: *id,
                        min_width,
                        min_height,
                        max_width: w.max_width.map(|v| v as u32),
                        max_height: w.max_height.map(|v| v as u32),
                        flex_grow: w.flex_grow.unwrap_or(0.0),
                        flex_shrink: w.flex_shrink.unwrap_or(1.0),
                    }
                })
            })
            .collect();

        let rects = layout::layout(container_rect, spec, &items, gap);

        for (child_id, rect) in &rects {
            if let Some(child) = self.widgets.get(child_id) {
                if child.kind.is_none() {
                    continue;
                }
                let width_changed = child.width.map(|w| w as u32 != rect.width).unwrap_or(true);
                let height_changed = child
                    .height
                    .map(|h| h as u32 != rect.height)
                    .unwrap_or(true);

                if width_changed || height_changed {
                    let mut updates = graph::map();
                    if width_changed {
                        updates.insert(canon::WIDTH, Value::U64(rect.width as u64));
                    }
                    if height_changed {
                        updates.insert(canon::HEIGHT, Value::U64(rect.height as u64));
                    }
                    graph::fiat(Some(*child_id), canon::WIDGET, updates);
                }
            }
        }

        for (child_id, rect) in rects {
            if let Some(child) = self.widgets.get(&child_id) {
                self.draw_widget_recursive(
                    scene,
                    window_id,
                    child,
                    rect.x,
                    rect.y,
                    rect.width as i32,
                    rect.height as i32,
                    windows,
                );
            }
        }
    }

    fn draw_widget_recursive(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        widget: &Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        windows: &BTreeMap<Uuid, WindowSurface>,
    ) -> i32 {
        if !widget.visible {
            return 0;
        }

        if let Some(bytes) = &widget.bitmap {
            if let (Some(bw), Some(bh)) = (widget.width, widget.height) {
                let expected_len = (bw * bh) as usize;
                let mut pixels = Vec::with_capacity(expected_len);
                for chunk in bytes.chunks(4) {
                    if pixels.len() >= expected_len {
                        break;
                    }
                    if chunk.len() == 4 {
                        let b = chunk[0] as u32; // Blue
                        let g = chunk[1] as u32; // Green
                        let r = chunk[2] as u32; // Red
                        let a = chunk[3] as u32; // Alpha
                                                 // ARGB
                        let val = (a << 24) | (r << 16) | (g << 8) | b;
                        pixels.push(val);
                    } else {
                        pixels.push(0);
                    }
                }

                // Pad with transparent pixels if the source data is smaller than the declared dimensions
                while pixels.len() < expected_len {
                    pixels.push(0);
                }

                let bmp = Arc::new(Bitmap::new(bw as usize, bh as usize, pixels));

                scene.push(SceneItem::BlitImage {
                    rect: Rect::new(x, y, w as u32, h as u32),
                    image: bmp,
                    repeat: false,
                    offset: (0, 0),
                });

                return h;
            }
        }

        let mut drawn_height = 0;

        if widget.role == ROLE_TOOLBAR || widget.role == "toolbar" {
            drawn_height = TOOLBAR_HEIGHT;
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: THEME.client_bg,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y + drawn_height - 1, w as u32, 1),
                color: THEME.frame_shadow,
            });

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            let toolbar_end = x + w;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    if child_x + btn_w > toolbar_end {
                        break;
                    }

                    self.draw_toolbar_button(scene, child, child_x, btn_y, btn_w, btn_h);
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
        } else if widget.role == ROLE_CONTAINER_VERTICAL
            || widget.role == "window_root"
            || widget.role == ROLE_EDITOR_ROOT
        {
            if widget.role == ROLE_EDITOR_ROOT {
                self.draw_surface_content(scene, window_id, x, y, w, h, windows);
            }

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let (direction, wrap, justify, align) = if let Some(dir) = widget.flex_direction {
                (
                    dir,
                    widget.flex_wrap.unwrap_or_default(),
                    widget.justify_content.unwrap_or_default(),
                    widget.align_items.unwrap_or(AlignItems::Start),
                )
            } else {
                (
                    FlexDirection::Column,
                    FlexWrap::NoWrap,
                    JustifyContent::Start,
                    AlignItems::Stretch,
                )
            };

            let spec = LayoutSpec::Flex {
                direction,
                wrap,
                justify,
                align,
            };

            let gap = widget.gap.unwrap_or(0);
            self.layout_and_draw_children(
                scene,
                window_id,
                Rect::new(x, y, w as u32, h as u32),
                &children,
                spec,
                gap,
                windows,
            );
            drawn_height = h;
        } else if widget.role == "button" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(30);
            self.draw_toolbar_button(scene, widget, x, y, w, drawn_height);
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 4, (w - 8) as u32, (drawn_height - 8) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        } else if widget.role == "listbox_default" || widget.role == "primary_list" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(100);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: Rgba::new(255, 255, 255, 255),
            });
            self.draw_rect_outline(scene, x, y, w, drawn_height, BTN_BORDER);

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y + 2;
            let mut remaining_h = drawn_height - 4;
            let child_w = w - 4;
            let child_x = x + 2;

            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let child_h = self.draw_widget_recursive(
                        scene,
                        window_id,
                        child,
                        child_x,
                        child_y,
                        child_w,
                        remaining_h,
                        windows,
                    );
                    child_y += child_h;
                    remaining_h -= child_h;
                }
            }
        } else if widget.role == "scrollbar_thumb" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(30);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: BTN_FACE,
            });
            self.draw_rect_outline(scene, x, y, w, drawn_height, BTN_BORDER);
        } else if widget.role == "thing_tile" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(100);
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x, y, w as u32, drawn_height as u32),
                color: BTN_FACE,
            });
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 4, (w - 8) as u32, (drawn_height - 8) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        } else if widget.role == "list_item" {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(20);
            if let Some(label) = &widget.label {
                scene.push(SceneItem::DrawTextBlock {
                    rect: Rect::new(x + 4, y + 2, (w - 8) as u32, (drawn_height - 4) as u32),
                    text: label.clone(),
                    color: COLOR_TEXT,
                    scroll_offset: 0,
                });
            }
        }

        drawn_height
    }

    fn draw_toolbar_button(
        &self,
        scene: &mut Scene,
        widget: &Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w.max(0) as u32, h.max(0) as u32),
            color: BTN_FACE,
        });
        self.draw_rect_outline(scene, x, y, w, h, BTN_BORDER);

        if w > 2 && h > 2 {
            let highlight = THEME.frame_hilight;
            let shadow = THEME.frame_shadow;
            let inner_width = (w - 2).max(0) as u32;
            let inner_height = (h - 2).max(0) as u32;

            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + 1, inner_width, 1),
                color: highlight,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + 1, 1, inner_height),
                color: highlight,
            });

            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + 1, y + h - 2, inner_width, 1),
                color: shadow,
            });
            scene.push(SceneItem::FillRect {
                rect: Rect::new(x + w - 2, y + 1, 1, inner_height),
                color: shadow,
            });
        }

        if let Some(icon_name) = &widget.icon {
            self.draw_toolbar_icon(scene, icon_name, x, y, w, h);
        }
    }

    fn draw_toolbar_icon(
        &self,
        scene: &mut Scene,
        icon_name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let color = BTN_GLYPH;
        let center_offset = |container: i32, item: i32| -> i32 { ((container - item).max(0)) / 2 };

        match icon_name {
            "save" => {
                let icon_size = 12;
                let icon_left = x + center_offset(w, icon_size);
                let icon_top = y + center_offset(h, icon_size);

                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, icon_size as u32, icon_size as u32),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left + 2, icon_top, (icon_size - 4).max(0) as u32, 4),
                    color: BTN_FACE,
                });
            }
            "undo" => {
                let icon_width = 12;
                let icon_height = 6;
                let icon_left = x + center_offset(w, icon_width);
                let icon_top = y + center_offset(h, icon_height);

                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top + 2, icon_width as u32, 2),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, 2, icon_height as u32),
                    color,
                });
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(icon_left, icon_top, 6, 2),
                    color,
                });
            }
            _ => {
                let icon_width = 8;
                let icon_height = FONT_HEIGHT as i32;
                let icon_left = x + center_offset(w, icon_width);
                let icon_top = y + center_offset(h, icon_height);
                let fallback_char = icon_name.chars().next().unwrap_or('?');

                scene.push(SceneItem::DrawText {
                    origin: (icon_left, icon_top),
                    text: fallback_char.to_string(),
                    color,
                    max_width: Some(w.max(0) as u32),
                });
            }
        }
    }

    fn draw_rect_outline(&self, scene: &mut Scene, x: i32, y: i32, w: i32, h: i32, color: Rgba) {
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, 1),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y + h - 1, w as u32, 1),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, 1, h as u32),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x + w - 1, y, 1, h as u32),
            color,
        });
    }

    fn draw_surface_content(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        windows: &BTreeMap<Uuid, WindowSurface>,
    ) {
        let Some(surface) = windows.get(&window_id) else {
            return;
        };

        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, h as u32),
            color: THEME.client_bg,
        });

        if let Some(bmp) = &surface.bitmap {
            scene.push(SceneItem::BlitImage {
                rect: Rect::new(x, y, w as u32, h as u32),
                image: bmp.clone(),
                repeat: false,
                offset: (0, 0),
            });
        } else if !surface.text.is_empty() {
            scene.push(SceneItem::DrawTextBlock {
                rect: Rect::new(x, y, w as u32, h as u32),
                text: surface.text.clone(),
                color: COLOR_TEXT,
                scroll_offset: surface.scroll_y,
            });

            let is_active = surface.window.active; // || self.active_window == Some(window_id); // Need active window info
            if is_active && surface.caret.visible {
                let cx = surface.caret.x;
                let cy = surface.caret.y;
                let ch = surface.caret.height;

                // Draw caret
                scene.push(SceneItem::FillRect {
                    rect: Rect::new(x + cx, y + cy, 2, ch as u32),
                    color: COLOR_TEXT,
                });
            }
        }
    }

    pub fn get_widget_height(&self, widget: &Widget, w: i32, h: i32) -> i32 {
        if !widget.visible {
            return 0;
        }
        if widget.role == ROLE_TOOLBAR {
            return TOOLBAR_HEIGHT;
        } else if widget.role == ROLE_TOOLBAR_BUTTON || widget.role == "toolbar_button" {
            return widget.height.map(|v| v as i32).unwrap_or(32);
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();
            let mut height = 0;
            let mut remaining_h = h;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let ch = self.get_widget_height(child, w, remaining_h);
                    height += ch;
                    remaining_h -= ch;
                }
            }
            return height;
        } else if widget.role == ROLE_EDITOR_ROOT {
            return h;
        }
        0
    }

    pub fn hit_test_widgets(
        &self,
        window_id: Uuid,
        layout: &WindowLayout,
        metrics: &ContentMetrics,
        mx: i32,
        my: i32,
    ) -> Option<Uuid> {
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(window_id))
            .map(|w| w.id)
            .collect();

        // Check absolute positioned widgets first (like scrollbars)
        for widget_id in &root_widgets {
            if let Some(widget) = self.widgets.get(widget_id) {
                if let (Some(x), Some(y), Some(w), Some(h)) =
                    (widget.x, widget.y, widget.width, widget.height)
                {
                    let x = x as i32;
                    let y = y as i32;
                    let w = w as i32;
                    let h = h as i32;
                    if mx >= x && mx < x + w && my >= y && my < y + h {
                        return Some(*widget_id);
                    }
                }
            }
        }

        let y_offset = layout.client_y;
        let x_offset = layout.client_x;
        let widget_width = if metrics.content_rect.width > 0 {
            metrics.content_rect.width as i32
        } else {
            layout.client_w
        };
        let width = widget_width.max(0);
        let height = layout.client_h;

        for widget_id in root_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    continue;
                }

                if let Some(hit) = self
                    .hit_test_widget_recursive(widget, x_offset, y_offset, width, height, mx, my)
                {
                    return Some(hit);
                }
            }
        }
        None
    }

    fn hit_test_widget_recursive(
        &self,
        widget: &Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        mx: i32,
        my: i32,
    ) -> Option<Uuid> {
        if !widget.visible {
            return None;
        }

        let height = self.get_widget_height(widget, w, h);

        if mx < x || mx >= x + w || my < y || my >= y + height {
            return None;
        }

        if widget.role == ROLE_TOOLBAR {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            let toolbar_end = x + w;
            let drawn_height = TOOLBAR_HEIGHT;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    if child_x + btn_w > toolbar_end {
                        break;
                    }

                    if mx >= child_x && mx < child_x + btn_w && my >= btn_y && my < btn_y + btn_h {
                        return Some(child.id);
                    }
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
            return Some(widget.id);
        } else if widget.role == ROLE_CONTAINER_VERTICAL
            || widget.role == "window_root"
            || widget.role == ROLE_EDITOR_ROOT
        {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y;
            let mut remaining_h = h;

            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let child_h = self.get_widget_height(child, w, remaining_h);
                    if let Some(hit) =
                        self.hit_test_widget_recursive(child, x, child_y, w, child_h, mx, my)
                    {
                        return Some(hit);
                    }
                    child_y += child_h;
                    remaining_h -= child_h;
                }
            }
            return Some(widget.id);
        } else if widget.role == "listbox_default" || widget.role == "primary_list" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget.id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y + 2;
            let mut remaining_h = height - 4;
            let child_w = w - 4;
            let child_x = x + 2;

            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let child_h = self.get_widget_height(child, child_w, remaining_h);
                    if let Some(hit) = self.hit_test_widget_recursive(
                        child, child_x, child_y, child_w, child_h, mx, my,
                    ) {
                        return Some(hit);
                    }
                    child_y += child_h;
                    remaining_h -= child_h;
                }
            }
            return Some(widget.id);
        }

        Some(widget.id)
    }

    pub fn collect_focusable_widgets(&self, parent_id: Uuid, list: &mut Vec<Uuid>) {
        let mut children: Vec<&Widget> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(parent_id))
            .collect();

        // Sort by ID for stability (creation order)
        children.sort_by_key(|w| w.id);

        for child in children {
            if child.focusable {
                list.push(child.id);
            }
            self.collect_focusable_widgets(child.id, list);
        }
    }

    pub fn draw_debug_widgets(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        win_x: i32,
        win_y: i32,
        win_w: i32,
        win_h: i32,
        scroll_y: i32,
    ) {
        let root_widgets: Vec<Uuid> = self
            .widgets
            .values()
            .filter(|w| w.parent == Some(window_id))
            .map(|w| w.id)
            .collect();

        if root_widgets.is_empty() {
            return;
        }

        let layout = match compute_window_layout(win_x, win_y, win_w, win_h) {
            Some(l) => l,
            None => return,
        };

        let mut y_offset = layout.client_y;
        let x_offset = layout.client_x;
        let width = layout.client_w;
        let mut remaining_h = layout.client_h;

        let mut relative_widgets = Vec::new();
        let mut overlay_widgets = Vec::new();

        for widget_id in root_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if widget.x.is_some() && widget.y.is_some() {
                    overlay_widgets.push(widget_id);
                } else {
                    relative_widgets.push(widget_id);
                }
            }
        }

        for widget_id in relative_widgets {
            let child_h = self.draw_debug_widget_recursive(
                scene,
                window_id,
                widget_id,
                x_offset,
                y_offset,
                width,
                remaining_h,
                scroll_y,
            );
            y_offset += child_h;
            remaining_h = remaining_h.saturating_sub(child_h);
        }

        for widget_id in overlay_widgets {
            if let Some(widget) = self.widgets.get(&widget_id) {
                if let (Some(wx), Some(wy), Some(ww), Some(wh)) =
                    (widget.x, widget.y, widget.width, widget.height)
                {
                    self.draw_debug_widget_recursive(
                        scene,
                        window_id,
                        widget_id,
                        win_x + wx as i32,
                        win_y + wy as i32,
                        ww as i32,
                        wh as i32,
                        scroll_y,
                    );
                }
            }
        }
    }

    fn draw_debug_widget_recursive(
        &self,
        scene: &mut Scene,
        window_id: Uuid,
        widget_id: Uuid,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        scroll_y: i32,
    ) -> i32 {
        let Some(widget) = self.widgets.get(&widget_id) else {
            return 0;
        };
        if !widget.visible {
            return 0;
        }

        let mut drawn_height = 0;

        if widget.role == ROLE_TOOLBAR {
            drawn_height = TOOLBAR_HEIGHT;
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_x = x;
            for child_id in children {
                if let Some(child) = self.widgets.get(&child_id) {
                    let btn_w = TOOLBAR_BUTTON_SIZE;
                    let btn_h = TOOLBAR_BUTTON_SIZE;
                    let btn_y = y + (drawn_height - btn_h) / 2;

                    self.draw_debug_widget_box(scene, child, child_x, btn_y, btn_w, btn_h);
                    child_x += btn_w + TOOLBAR_BUTTON_SPACING;
                }
            }
        } else if widget.role == ROLE_CONTAINER_VERTICAL || widget.role == "window_root" {
            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y;
            let mut remaining_h = h;

            for child_id in children {
                let child_h = self.draw_debug_widget_recursive(
                    scene,
                    window_id,
                    child_id,
                    x,
                    child_y,
                    w,
                    remaining_h,
                    scroll_y,
                );
                child_y += child_h;
                drawn_height += child_h;
                remaining_h = remaining_h.saturating_sub(child_h);
            }
        } else if widget.role == ROLE_EDITOR_ROOT {
            drawn_height = h;

            let children: Vec<Uuid> = self
                .widgets
                .values()
                .filter(|w| w.parent == Some(widget_id))
                .map(|w| w.id)
                .collect();

            let mut child_y = y - scroll_y;
            // Give children plenty of space to draw themselves
            let child_available_h = 10000;

            for child_id in children {
                let child_h = self.draw_debug_widget_recursive(
                    scene,
                    window_id,
                    child_id,
                    x,
                    child_y,
                    w,
                    child_available_h,
                    scroll_y,
                );
                child_y += child_h;
            }
        } else {
            drawn_height = widget.height.map(|v| v as i32).unwrap_or(32);
        }

        self.draw_debug_widget_box(scene, widget, x, y, w, drawn_height);
        drawn_height
    }

    fn draw_debug_widget_box(
        &self,
        scene: &mut Scene,
        widget: &Widget,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let color = Rgba::new(0xFF, 0x00, 0xFF, 0x00); // Green
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, w as u32, 2),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y + h - 2, w as u32, 2),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x, y, 2, h as u32),
            color,
        });
        scene.push(SceneItem::FillRect {
            rect: Rect::new(x + w - 2, y, 2, h as u32),
            color,
        });

        if let Some(bitmap) = &widget.bitmap {
            if let Some(bmp) = decode_bmp(bitmap) {
                scene.push(SceneItem::BlitImage {
                    rect: Rect::new(x, y, w as u32, h as u32),
                    image: Arc::new(bmp),
                    repeat: false,
                    offset: (0, 0),
                });
            }
        }

        if Some(widget.id) == self.active_widget {
            scene.push(SceneItem::HatchRect {
                rect: Rect::new(x, y, w as u32, h as u32),
                color: Rgba::new(0xFF, 0xFF, 0xFF, 0x00), // Yellow
                spacing: 4,
            });
        }

        scene.push(SceneItem::DrawText {
            origin: (x + 2, y + 2),
            text: widget.role.clone(),
            color,
            max_width: Some(w.saturating_sub(4) as u32),
        });
    }
}
