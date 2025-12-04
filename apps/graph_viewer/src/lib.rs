#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use font8x8::legacy::BASIC_FONTS;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter};
use uuid::Uuid;

const CLOUDS_BMP: &[u8] = include_bytes!("../../../clouds.bmp");
const ICON_SIZE: u32 = 48;
const ICON_PADDING: u32 = 28;
const ICON_TEXT_HEIGHT: u32 = 10;
const EDGE_THICKNESS: i32 = 2;
const EDGE_HIT_RADIUS: f32 = 6.0;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    src: Uuid,
    dst: Uuid,
    pred: String,
}

#[derive(Clone, Debug)]
struct EdgeVisual {
    key: EdgeKey,
}

#[derive(Clone, Debug)]
struct NodeVisual {
    thing: userland::graph::GraphThing,
    position: (i32, i32),
}

#[derive(Clone)]
struct DecodedBitmap {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

pub struct GraphViewerApp {
    window: WindowHandle,
    nodes: Vec<NodeVisual>,
    edges: Vec<EdgeVisual>,
    selected_node: Option<Uuid>,
    selected_edge: Option<EdgeKey>,
    tile: DecodedBitmap,
    fb_size: (u32, u32),
    dirty: bool,
    watch_all: userland::watch::WatchId,
    watch_keys: userland::watch::WatchId,
    watch_mouse_move: userland::watch::WatchId,
    watch_mouse_buttons: userland::watch::WatchId,
    cursor: (i32, i32),
}

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let fb_info = userland::sys::fb_info();
        let fb_size = fb_info
            .map(|info| (info.width as u32, info.height as u32))
            .unwrap_or((1024, 768));

        let mut window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Graph Viewer".to_string(),
            x: 0,
            y: 0,
            width: fb_size.0 as u64,
            height: fb_size.1 as u64,
            z: 0,
            visible: true,
            target: None,
            active: true,
            is_root: true,
            mode_index: Some(0),
            window_rect: None,
        };

        let window = ctx.create_window_with(window_fields.clone());

        let watch_all = ctx.watch_graph(ThingFilter {
            kind: None,
            id: None,
        });
        let watch_keys = ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });
        let watch_mouse_move = ctx.watch_graph(ThingFilter {
            kind: Some(canon::MOUSE_MOVE),
            id: None,
        });
        let watch_mouse_buttons = ctx.watch_graph(ThingFilter {
            kind: Some(canon::MOUSE_BUTTON),
            id: None,
        });

        let tile = decode_bmp(CLOUDS_BMP).unwrap_or_else(fallback_tile);

        let mut app = GraphViewerApp {
            window,
            nodes: Vec::new(),
            edges: Vec::new(),
            selected_node: None,
            selected_edge: None,
            tile,
            fb_size,
            dirty: true,
            watch_all,
            watch_keys,
            watch_mouse_move,
            watch_mouse_buttons,
            cursor: (0, 0),
        };

        app.refresh_graph();
        app.layout_nodes();
        app.selected_node = app.nodes.first().map(|n| n.thing.id);
        app
    }

    fn on_event(&mut self, ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } => {
                if watch == self.watch_all {
                    self.upsert_node(thing);
                    self.dirty = true;
                } else if watch == self.watch_keys {
                    self.handle_key_event(&thing);
                } else if watch == self.watch_mouse_move {
                    self.update_cursor(&thing);
                } else if watch == self.watch_mouse_buttons {
                    self.handle_mouse_button(&thing);
                }
            }
            AppEvent::Edge { watch, edge } => {
                if watch == self.watch_all {
                    self.add_edge(edge);
                    self.dirty = true;
                }
            }
        }

        if self.dirty {
            self.layout_nodes();
            self.draw(ctx);
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        ctx.begin_tick();
        if self.dirty {
            self.draw(ctx);
            self.dirty = false;
        }
        ctx.flush(tick);
    }
}

impl GraphViewerApp {
    fn refresh_graph(&mut self) {
        let pattern = graph::NodePattern::default();
        let nodes = graph::get_nodes(pattern);
        self.nodes = nodes
            .into_iter()
            .map(|thing| NodeVisual {
                thing,
                position: (0, 0),
            })
            .collect();

        self.edges = derive_edges_from_nodes(&self.nodes);
    }

    fn upsert_node(&mut self, thing: userland::graph::GraphThing) {
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.thing.id == thing.id) {
            existing.thing = thing.clone();
        } else {
            self.nodes.push(NodeVisual {
                thing: thing.clone(),
                position: (0, 0),
            });
        }

        self.merge_edges_from_node(&thing);
    }

    fn add_edge(&mut self, edge: userland::graph::GraphEdge) {
        let key = EdgeKey {
            src: edge.src,
            dst: edge.dst,
            pred: edge.pred,
        };

        if !self.edges.iter().any(|e| e.key == key) {
            self.edges.push(EdgeVisual { key });
        }
    }

    fn merge_edges_from_node(&mut self, thing: &userland::graph::GraphThing) {
        let mut new_edges = derive_edges_from_nodes(&[NodeVisual {
            thing: thing.clone(),
            position: (0, 0),
        }]);
        self.edges.extend(new_edges.into_iter().filter(|candidate| {
            !self
                .edges
                .iter()
                .any(|existing| existing.key == candidate.key)
        }));
    }

    fn layout_nodes(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let (width, height) = self.fb_size;
        let columns = core::cmp::max(1, (width / (ICON_SIZE + ICON_PADDING)).max(1) as usize);
        let start_x = ICON_PADDING as i32;
        let start_y = ICON_PADDING as i32;
        let step_x = (ICON_SIZE + ICON_PADDING) as i32;
        let step_y = (ICON_SIZE + ICON_PADDING + ICON_TEXT_HEIGHT) as i32;

        for (idx, node) in self.nodes.iter_mut().enumerate() {
            let col = (idx % columns) as i32;
            let row = (idx / columns) as i32;
            let x = start_x + col * step_x;
            let y = start_y + row * step_y;
            node.position = (x, y);
        }
    }

    fn draw(&mut self, ctx: &mut AppContext<'_>) {
        let bitmap = self.render_scene();
        ctx.draw_bitmap(&self.window, &bitmap);
    }

    fn render_scene(&self) -> Vec<u8> {
        let (width, height) = self.fb_size;
        let mut canvas = tile_canvas(&self.tile, width, height);

        let positions: BTreeMap<Uuid, (i32, i32)> = self
            .nodes
            .iter()
            .map(|n| (n.thing.id, n.position))
            .collect();

        for edge in &self.edges {
            let Some(start) = positions.get(&edge.key.src) else {
                continue;
            };
            let Some(end) = positions.get(&edge.key.dst) else {
                continue;
            };
            let start_center = center_of(*start);
            let end_center = center_of(*end);

            let color = if Some(&edge.key) == self.selected_edge.as_ref() {
                0xFFFFC000
            } else {
                0xFF5E81AC
            };
            draw_line(
                &mut canvas.pixels,
                canvas.width,
                canvas.height,
                start_center,
                end_center,
                color,
                EDGE_THICKNESS,
            );
        }

        for node in &self.nodes {
            let highlight = self.selected_node == Some(node.thing.id);
            draw_icon(&mut canvas, node, highlight);
        }

        encode_bmp(&canvas)
    }

    fn handle_key_event(&mut self, thing: &userland::graph::GraphThing) {
        let Some(key_sym) = thing.fields.get(&canon::KEY).and_then(|v| v.as_symbol()) else {
            return;
        };
        if let Some(ch) = key_to_char(key_sym) {
            match ch {
                'n' | 'N' => self.select_next_node(),
                'e' | 'E' => self.select_next_edge(),
                _ => {}
            }
        }
    }

    fn handle_mouse_button(&mut self, thing: &userland::graph::GraphThing) {
        let down = thing
            .fields
            .get(&canon::MOUSE_DOWN)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !down {
            return;
        }
        let x = thing
            .fields
            .get(&canon::MOUSE_X)
            .and_then(|v| v.as_i64())
            .unwrap_or(self.cursor.0 as i64) as i32;
        let y = thing
            .fields
            .get(&canon::MOUSE_Y)
            .and_then(|v| v.as_i64())
            .unwrap_or(self.cursor.1 as i64) as i32;
        self.cursor = (x, y);
        if self.try_select_node_at(self.cursor) {
            self.selected_edge = None;
            self.dirty = true;
            return;
        }

        if self.try_select_edge_at(self.cursor) {
            self.selected_node = None;
            self.dirty = true;
        }
    }

    fn update_cursor(&mut self, thing: &userland::graph::GraphThing) {
        let x = thing
            .fields
            .get(&canon::MOUSE_X)
            .and_then(|v| v.as_i64())
            .unwrap_or(self.cursor.0 as i64) as i32;
        let y = thing
            .fields
            .get(&canon::MOUSE_Y)
            .and_then(|v| v.as_i64())
            .unwrap_or(self.cursor.1 as i64) as i32;
        self.cursor = (x, y);
    }

    fn select_next_node(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let current_idx = self
            .selected_node
            .and_then(|id| self.nodes.iter().position(|n| n.thing.id == id))
            .unwrap_or(0);
        let next = (current_idx + 1) % self.nodes.len();
        self.selected_node = Some(self.nodes[next].thing.id);
        self.selected_edge = None;
        self.dirty = true;
    }

    fn select_next_edge(&mut self) {
        if self.edges.is_empty() {
            return;
        }
        let current_idx = self
            .selected_edge
            .as_ref()
            .and_then(|edge| self.edges.iter().position(|e| &e.key == edge))
            .unwrap_or(0);
        let next = (current_idx + 1) % self.edges.len();
        self.selected_edge = Some(self.edges[next].key.clone());
        self.selected_node = None;
        self.dirty = true;
    }

    fn try_select_node_at(&mut self, point: (i32, i32)) -> bool {
        for node in &self.nodes {
            let (x, y) = node.position;
            let rect = (
                x,
                y,
                (ICON_SIZE) as i32,
                (ICON_SIZE + ICON_TEXT_HEIGHT) as i32,
            );
            if point.0 >= rect.0
                && point.0 < rect.0 + rect.2
                && point.1 >= rect.1
                && point.1 < rect.1 + rect.3
            {
                self.selected_node = Some(node.thing.id);
                return true;
            }
        }
        false
    }

    fn try_select_edge_at(&mut self, point: (i32, i32)) -> bool {
        let positions: BTreeMap<Uuid, (i32, i32)> = self
            .nodes
            .iter()
            .map(|n| (n.thing.id, n.position))
            .collect();
        let mut closest: Option<EdgeKey> = None;
        let mut best_distance = f32::MAX;
        for edge in &self.edges {
            let Some(start) = positions.get(&edge.key.src) else {
                continue;
            };
            let Some(end) = positions.get(&edge.key.dst) else {
                continue;
            };
            let dist = distance_to_segment(point, center_of(*start), center_of(*end));
            if dist < best_distance && dist <= EDGE_HIT_RADIUS {
                best_distance = dist;
                closest = Some(edge.key.clone());
            }
        }

        if let Some(edge) = closest {
            self.selected_edge = Some(edge);
            return true;
        }
        false
    }
}

fn label_for(thing: &userland::graph::GraphThing) -> String {
    if let Some(Value::Text(name)) = thing.fields.get(&canon::NAME) {
        return name.clone();
    }
    let kind_str: String = thing.kind.into();
    if !kind_str.is_empty() {
        return kind_str;
    }
    format!("{}", thing.id)
}

fn derive_edges_from_nodes(nodes: &[NodeVisual]) -> Vec<EdgeVisual> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for node in nodes {
        for (key, value) in node.thing.fields.iter() {
            if let Some(target) = value.as_uuid() {
                let edge = EdgeKey {
                    src: node.thing.id,
                    dst: target,
                    pred: String::from(*key),
                };
                if seen.insert(edge.clone()) {
                    out.push(EdgeVisual { key: edge });
                }
            }
        }
    }
    out
}

fn center_of(origin: (i32, i32)) -> (i32, i32) {
    (
        origin.0 + (ICON_SIZE as i32 / 2),
        origin.1 + (ICON_SIZE as i32 / 2),
    )
}

fn draw_icon(canvas: &mut DecodedBitmap, node: &NodeVisual, highlight: bool) {
    let base_color = if highlight { 0xFF5BBF6A } else { 0xFF1E88E5 };
    draw_rect(
        &mut canvas.pixels,
        canvas.width,
        canvas.height,
        node.position,
        ICON_SIZE,
        ICON_SIZE,
        base_color,
    );

    let border_color = if highlight { 0xFFFFD166 } else { 0xFF0D47A1 };
    stroke_rect(
        &mut canvas.pixels,
        canvas.width,
        canvas.height,
        node.position,
        ICON_SIZE,
        ICON_SIZE,
        border_color,
    );

    let text_origin = (node.position.0, node.position.1 + ICON_SIZE as i32 + 2);
    draw_text(
        &mut canvas.pixels,
        canvas.width,
        canvas.height,
        text_origin,
        &label_for(&node.thing),
        0xFF0C0C0C,
    );
}

fn draw_rect(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    origin: (i32, i32),
    rect_w: u32,
    rect_h: u32,
    color: u32,
) {
    for y in 0..rect_h {
        for x in 0..rect_w {
            let px = origin.0 + x as i32;
            let py = origin.1 + y as i32;
            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                continue;
            }
            let idx = py as usize * width as usize + px as usize;
            pixels[idx] = color;
        }
    }
}

fn stroke_rect(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    origin: (i32, i32),
    rect_w: u32,
    rect_h: u32,
    color: u32,
) {
    let end_x = origin.0 + rect_w as i32 - 1;
    let end_y = origin.1 + rect_h as i32 - 1;
    for x in origin.0..=end_x {
        set_pixel(pixels, width, height, (x, origin.1), color);
        set_pixel(pixels, width, height, (x, end_y), color);
    }
    for y in origin.1..=end_y {
        set_pixel(pixels, width, height, (origin.0, y), color);
        set_pixel(pixels, width, height, (end_x, y), color);
    }
}

fn set_pixel(pixels: &mut [u32], width: u32, height: u32, pos: (i32, i32), color: u32) {
    if pos.0 < 0 || pos.1 < 0 || pos.0 >= width as i32 || pos.1 >= height as i32 {
        return;
    }
    let idx = pos.1 as usize * width as usize + pos.0 as usize;
    pixels[idx] = color;
}

fn draw_line(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    start: (i32, i32),
    end: (i32, i32),
    color: u32,
    thickness: i32,
) {
    let dx = (end.0 - start.0).abs();
    let dy = -(end.1 - start.1).abs();
    let sx = if start.0 < end.0 { 1 } else { -1 };
    let sy = if start.1 < end.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = start.0;
    let mut y = start.1;

    loop {
        for oy in -thickness..=thickness {
            for ox in -thickness..=thickness {
                set_pixel(pixels, width, height, (x + ox, y + oy), color);
            }
        }
        if x == end.0 && y == end.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn draw_text(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    origin: (i32, i32),
    text: &str,
    color: u32,
) {
    let mut x = origin.0;
    let y = origin.1;
    for ch in text.chars().take(10) {
        if let Some(glyph) = BASIC_FONTS.get(ch as u8) {
            for (row_idx, row) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if row & (1 << col) != 0 {
                        set_pixel(pixels, width, height, (x + col, y + row_idx as i32), color);
                    }
                }
            }
        }
        x += 8;
    }
}

fn distance_to_segment(point: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f32 {
    let px = point.0 as f32;
    let py = point.1 as f32;
    let ax = a.0 as f32;
    let ay = a.1 as f32;
    let bx = b.0 as f32;
    let by = b.1 as f32;

    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len_sq = abx * abx + aby * aby;
    if ab_len_sq == 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (apx * abx + apy * aby) / ab_len_sq;
    let t_clamped = t.clamp(0.0, 1.0);
    let proj_x = ax + t_clamped * abx;
    let proj_y = ay + t_clamped * aby;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn tile_canvas(tile: &DecodedBitmap, width: u32, height: u32) -> DecodedBitmap {
    let mut pixels = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let tx = (x % tile.width) as usize;
            let ty = (y % tile.height) as usize;
            let tile_idx = ty * tile.width as usize + tx;
            let canvas_idx = y as usize * width as usize + x as usize;
            pixels[canvas_idx] = tile.pixels[tile_idx];
        }
    }
    DecodedBitmap {
        width,
        height,
        pixels,
    }
}

fn decode_bmp(data: &[u8]) -> Option<DecodedBitmap> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return None;
    }
    let data_offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(data[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(data[22..26].try_into().ok()?);
    let planes = u16::from_le_bytes(data[26..28].try_into().ok()?);
    let bpp = u16::from_le_bytes(data[28..30].try_into().ok()?);
    let compression = u32::from_le_bytes(data[30..34].try_into().ok()?);

    if planes != 1 || bpp != 24 || compression != 0 {
        return None;
    }

    let width_u = width.unsigned_abs() as usize;
    let height_u = height.unsigned_abs() as usize;
    let row_stride = ((bpp as usize * width_u + 31) / 32) * 4;
    let mut pixels = Vec::with_capacity(width_u * height_u);

    for row in 0..height_u {
        let bmp_row = if height < 0 { row } else { height_u - 1 - row };
        let row_start = data_offset + bmp_row * row_stride;
        let row_end = row_start + row_stride;
        let row_data = data.get(row_start..row_end)?;
        for col in 0..width_u {
            let idx = col * 3;
            if idx + 2 >= row_data.len() {
                break;
            }
            let b = row_data[idx] as u32;
            let g = row_data[idx + 1] as u32;
            let r = row_data[idx + 2] as u32;
            pixels.push(0xFF000000 | (r << 16) | (g << 8) | b);
        }
    }

    Some(DecodedBitmap {
        width: width_u as u32,
        height: height_u as u32,
        pixels,
    })
}

fn encode_bmp(bitmap: &DecodedBitmap) -> Vec<u8> {
    let width = bitmap.width as usize;
    let height = bitmap.height as usize;
    let row_stride = ((24 * width + 31) / 32) * 4;
    let pixel_array_size = row_stride * height;
    let file_size = 54 + pixel_array_size;

    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(54u32).to_le_bytes());
    out.extend_from_slice(&(40u32).to_le_bytes());
    out.extend_from_slice(&(bitmap.width as i32).to_le_bytes());
    out.extend_from_slice(&(bitmap.height as i32).to_le_bytes());
    out.extend_from_slice(&(1u16).to_le_bytes());
    out.extend_from_slice(&(24u16).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(pixel_array_size as u32).to_le_bytes());
    out.extend_from_slice(&(2835u32).to_le_bytes());
    out.extend_from_slice(&(2835u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);

    let padding = vec![0u8; row_stride - width * 3];
    for row in (0..height).rev() {
        for col in 0..width {
            let idx = row * width + col;
            let pixel = bitmap.pixels[idx];
            let b = (pixel & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let r = ((pixel >> 16) & 0xFF) as u8;
            out.push(b);
            out.push(g);
            out.push(r);
        }
        out.extend_from_slice(&padding);
    }
    out
}

fn fallback_tile() -> DecodedBitmap {
    let width = 32;
    let height = 32;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let shade = if (x / 8 + y / 8) % 2 == 0 {
                0xFFEEEEEE
            } else {
                0xFFD0D0D0
            };
            pixels.push(shade);
        }
    }
    DecodedBitmap {
        width: width as u32,
        height: height as u32,
        pixels,
    }
}

fn key_to_char(sym: canon::Symbol) -> Option<char> {
    if let Some(ch) = char::from_u32(sym.0) {
        Some(ch)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_round_trips() {
        let tile = fallback_tile();
        let encoded = encode_bmp(&tile);
        let decoded = decode_bmp(&encoded).expect("decode fallback bmp");
        assert_eq!(tile.width, decoded.width);
        assert_eq!(tile.height, decoded.height);
        assert_eq!(tile.pixels, decoded.pixels);
    }
}
