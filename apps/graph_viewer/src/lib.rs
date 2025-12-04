#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use userland::canon;
use userland::graph::{self, NodePattern};
use userland::graphics::blend;
use userland::prelude::*;
use userland::watch::WatchId;
use uuid::Uuid;

const ICON_RADIUS: i32 = 22;
const EDGE_COLOR: u32 = 0xFF5A7184;
const EDGE_HIGHLIGHT: u32 = 0xFFFFD166;
const NODE_COLOR: u32 = 0xFF4C9AFF;
const NODE_HIGHLIGHT: u32 = 0xFFFFB347;
const NODE_TEXT_COLOR: u32 = 0xFFFFFFFF;

pub struct GraphViewerApp {
    window: WindowHandle,
    nodes: Vec<NodeVisual>,
    edges: Vec<EdgeVisual>,
    selection: Selection,
    graph_watch: WatchId,
    key_watch: WatchId,
    fb_width: u32,
    fb_height: u32,
    tile: Vec<u32>,
}

#[derive(Clone)]
struct NodeVisual {
    id: Uuid,
    label: String,
    position: (f32, f32),
}

#[derive(Clone)]
struct EdgeVisual {
    id: Uuid,
    src: Uuid,
    dst: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Selection {
    None,
    Node(usize),
    Edge(usize),
}

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let fb = userland::sys::fb_info();
        let fb_width = fb.map(|info| info.width as u32).unwrap_or(1024);
        let fb_height = fb.map(|info| info.height as u32).unwrap_or(768);

        let mut window = graph::Window {
            id: Uuid::nil(),
            title: "Graph Viewer".to_string(),
            x: 0,
            y: 0,
            width: fb_width as u64,
            height: fb_height as u64,
            z: 0,
            visible: true,
            target: None,
            active: true,
            window_rect: None,
        };

        window.window_rect = Some(graph::Rect {
            x: 0,
            y: 0,
            width: fb_width,
            height: fb_height,
        });

        let window = ctx.create_window_with(window);

        let graph_watch = ctx.watch_graph(ThingFilter {
            kind: None,
            id: None,
        });
        let key_watch = ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        let mut app = GraphViewerApp {
            window,
            nodes: Vec::new(),
            edges: Vec::new(),
            selection: Selection::None,
            graph_watch,
            key_watch,
            fb_width,
            fb_height,
            tile: load_cloud_tile(),
        };

        app.seed_nodes();
        app.selection = if app.nodes.is_empty() {
            Selection::None
        } else {
            Selection::Node(0)
        };

        app
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } if watch == self.graph_watch => {
                self.upsert_node(thing);
            }
            AppEvent::Edge { watch, edge } if watch == self.graph_watch => {
                self.upsert_edge(edge);
            }
            AppEvent::Thing { watch, thing } if watch == self.key_watch => {
                self.handle_key_event(&thing);
            }
            _ => {}
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        ctx.begin_tick();

        if let Some(window) = ctx.load_window(&self.window) {
            self.fb_width = window.width as u32;
            self.fb_height = window.height as u32;
        }

        self.layout_nodes();

        let mut pixels = vec![0u32; (self.fb_width * self.fb_height) as usize];
        tile_background(&self.tile, self.fb_width, self.fb_height, &mut pixels);

        self.draw_edges(&mut pixels);
        self.draw_nodes(&mut pixels);

        let mut bitmap_bytes = Vec::with_capacity(pixels.len() * 4);
        for px in pixels {
            bitmap_bytes.extend_from_slice(&px.to_le_bytes());
        }

        ctx.draw_bitmap(&self.window, &bitmap_bytes);
        self.render_legend(ctx);
        ctx.flush(tick);
    }
}

impl GraphViewerApp {
    fn seed_nodes(&mut self) {
        let things = graph::get_nodes(NodePattern::default());
        for thing in things {
            self.upsert_node(thing);
        }
    }

    fn upsert_node(&mut self, thing: graph::GraphThing) {
        if let Some(idx) = self.nodes.iter().position(|n| n.id == thing.id) {
            self.nodes[idx].label = label_for(&thing);
        } else {
            self.nodes.push(NodeVisual {
                id: thing.id,
                label: label_for(&thing),
                position: (0.0, 0.0),
            });
        }
    }

    fn upsert_edge(&mut self, edge: graph::GraphEdge) {
        if let Some(idx) = self.edges.iter().position(|e| e.id == edge.id) {
            self.edges[idx].src = edge.src;
            self.edges[idx].dst = edge.dst;
        } else {
            self.edges.push(EdgeVisual {
                id: edge.id,
                src: edge.src,
                dst: edge.dst,
            });
        }
    }

    fn handle_key_event(&mut self, thing: &graph::GraphThing) {
        let down = thing
            .fields
            .get(&canon::DOWN)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !down {
            return;
        }

        let Some(sym) = thing.fields.get(&canon::KEY).and_then(|v| v.as_symbol()) else {
            return;
        };

        if sym == canon::from_char('n') || sym == canon::from_char('N') {
            self.select_next_node();
        } else if sym == canon::from_char('p') || sym == canon::from_char('P') {
            self.select_prev_node();
        } else if sym == canon::from_char('e') || sym == canon::from_char('E') {
            self.select_next_edge();
        }
    }

    fn select_next_node(&mut self) {
        if self.nodes.is_empty() {
            self.selection = Selection::None;
            return;
        }

        let next = match self.selection {
            Selection::Node(idx) => (idx + 1) % self.nodes.len(),
            _ => 0,
        };
        self.selection = Selection::Node(next);
    }

    fn select_prev_node(&mut self) {
        if self.nodes.is_empty() {
            self.selection = Selection::None;
            return;
        }

        let next = match self.selection {
            Selection::Node(idx) if idx > 0 => idx - 1,
            Selection::Node(_) => self.nodes.len() - 1,
            _ => 0,
        };
        self.selection = Selection::Node(next);
    }

    fn select_next_edge(&mut self) {
        if self.edges.is_empty() {
            self.selection = Selection::None;
            return;
        }

        let next = match self.selection {
            Selection::Edge(idx) => (idx + 1) % self.edges.len(),
            _ => 0,
        };
        self.selection = Selection::Edge(next);
    }

    fn layout_nodes(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        let count = self.nodes.len() as f32;
        let cols = (count.sqrt().ceil() as usize).max(1);
        let rows = ((count / cols as f32).ceil() as usize).max(1);

        let spacing_x = (self.fb_width as i32 - ICON_RADIUS * 2) / cols.max(1) as i32;
        let spacing_y = (self.fb_height as i32 - ICON_RADIUS * 2) / rows.max(1) as i32;

        for (idx, node) in self.nodes.iter_mut().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let x = (spacing_x * col as i32 + ICON_RADIUS).max(ICON_RADIUS) as f32;
            let y = (spacing_y * row as i32 + ICON_RADIUS).max(ICON_RADIUS) as f32;
            node.position = (x, y);
        }
    }

    fn draw_edges(&self, pixels: &mut [u32]) {
        for (idx, edge) in self.edges.iter().enumerate() {
            let Some(src) = self.nodes.iter().find(|n| n.id == edge.src) else {
                continue;
            };
            let Some(dst) = self.nodes.iter().find(|n| n.id == edge.dst) else {
                continue;
            };

            let (sx, sy) = src.position;
            let (dx, dy) = dst.position;

            let highlight = matches!(self.selection, Selection::Edge(sel) if sel == idx);
            let color = if highlight {
                EDGE_HIGHLIGHT
            } else {
                EDGE_COLOR
            };

            draw_line(
                pixels,
                self.fb_width,
                self.fb_height,
                sx as i32,
                sy as i32,
                dx as i32,
                dy as i32,
                color,
            );
        }
    }

    fn draw_nodes(&self, pixels: &mut [u32]) {
        for (idx, node) in self.nodes.iter().enumerate() {
            let highlight = matches!(self.selection, Selection::Node(sel) if sel == idx);
            let color = if highlight {
                NODE_HIGHLIGHT
            } else {
                NODE_COLOR
            };

            draw_circle(
                pixels,
                self.fb_width,
                self.fb_height,
                node.position,
                ICON_RADIUS,
                color,
            );

            if let Some(initial) = node.label.chars().next() {
                draw_letter(
                    pixels,
                    self.fb_width,
                    self.fb_height,
                    node.position,
                    initial,
                    NODE_TEXT_COLOR,
                );
            }
        }
    }

    fn render_legend(&self, ctx: &mut AppContext<'_>) {
        ctx.draw_text(
            &self.window,
            format_args!("Graph nodes: {}\n", self.nodes.len()),
        );
        if let Selection::Node(idx) = self.selection {
            if let Some(node) = self.nodes.get(idx) {
                ctx.draw_text(
                    &self.window,
                    format_args!("Selected node: {}\n", node.label),
                );
            }
        } else if let Selection::Edge(idx) = self.selection {
            if let Some(edge) = self.edges.get(idx) {
                let src_label = self
                    .nodes
                    .iter()
                    .find(|n| n.id == edge.src)
                    .map(|n| n.label.clone())
                    .unwrap_or_else(|| "?".to_string());
                let dst_label = self
                    .nodes
                    .iter()
                    .find(|n| n.id == edge.dst)
                    .map(|n| n.label.clone())
                    .unwrap_or_else(|| "?".to_string());
                ctx.draw_text(
                    &self.window,
                    format_args!("Selected edge: {} → {}\n", src_label, dst_label),
                );
            }
        }

        ctx.draw_text(
            &self.window,
            format_args!("Use N/P to move between nodes, E to cycle edges."),
        );
    }
}

fn label_for(thing: &graph::GraphThing) -> String {
    thing
        .fields
        .get(&canon::LABEL)
        .and_then(|v| v.as_text())
        .map(ToString::to_string)
        .or_else(|| {
            thing
                .fields
                .get(&canon::NAME)
                .and_then(|v| v.as_text())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| format!("{}", thing.id))
}

fn tile_background(tile: &[u32], width: u32, height: u32, pixels: &mut [u32]) {
    if tile.is_empty() {
        return;
    }

    let tile_w = (tile.len() as f32).sqrt().round() as usize;
    let tile_h = if tile_w == 0 { 0 } else { tile.len() / tile_w };

    for y in 0..height as usize {
        for x in 0..width as usize {
            let tx = x % tile_w;
            let ty = y % tile_h;
            let color = tile[ty * tile_w + tx];
            pixels[y * width as usize + x] = color;
        }
    }
}

fn draw_line(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: u32,
) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        set_pixel(pixels, width, height, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_circle(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    center: (f32, f32),
    radius: i32,
    color: u32,
) {
    let (cx, cy) = (center.0 as i32, center.1 as i32);
    let mut x = radius;
    let mut y = 0;
    let mut err = 0;

    while x >= y {
        fill_circle_scanlines(pixels, width, height, cx, cy, x, y, color);
        y += 1;
        if err <= 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

fn fill_circle_scanlines(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    cx: i32,
    cy: i32,
    x: i32,
    y: i32,
    color: u32,
) {
    draw_horizontal_span(pixels, width, height, cx - x, cx + x, cy + y, color);
    draw_horizontal_span(pixels, width, height, cx - y, cx + y, cy + x, color);
    draw_horizontal_span(pixels, width, height, cx - x, cx + x, cy - y, color);
    draw_horizontal_span(pixels, width, height, cx - y, cx + y, cy - x, color);
}

fn draw_horizontal_span(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    x0: i32,
    x1: i32,
    y: i32,
    color: u32,
) {
    if y < 0 || y >= height as i32 {
        return;
    }

    let start = x0.max(0) as usize;
    let end = x1.min(width as i32 - 1) as usize;
    let row_start = y as usize * width as usize;
    for x in start..=end {
        let idx = row_start + x;
        pixels[idx] = blend(color, pixels[idx]);
    }
}

fn draw_letter(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    center: (f32, f32),
    ch: char,
    color: u32,
) {
    let glyph = font_bitmap(ch);
    let start_x = center.0 as i32 - (glyph[0].len() as i32 / 2);
    let start_y = center.1 as i32 - (glyph.len() as i32 / 2);

    for (row_idx, row) in glyph.iter().enumerate() {
        for (col_idx, &on) in row.iter().enumerate() {
            if on {
                let x = start_x + col_idx as i32;
                let y = start_y + row_idx as i32;
                set_pixel(pixels, width, height, x, y, color);
            }
        }
    }
}

fn set_pixel(pixels: &mut [u32], width: u32, height: u32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= width || y >= height {
        return;
    }
    let idx = (y * width + x) as usize;
    pixels[idx] = blend(color, pixels[idx]);
}

fn load_cloud_tile() -> Vec<u32> {
    let data = include_bytes!("../../clouds.bmp");
    decode_bmp(data).unwrap_or_else(|| vec![0xFF1E1E2E; 16 * 16])
}

fn decode_bmp(data: &[u8]) -> Option<Vec<u32>> {
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
    let stride = ((width_u * 3 + 3) / 4) * 4;

    if data_offset + stride.saturating_mul(height_u) > data.len() {
        return None;
    }

    let mut pixels = vec![0u32; width_u * height_u];
    for row in 0..height_u {
        let src_y = if height < 0 { row } else { height_u - 1 - row };
        let src_start = data_offset + src_y * stride;
        for col in 0..width_u {
            let idx = src_start + col * 3;
            let b = data[idx] as u32;
            let g = data[idx + 1] as u32;
            let r = data[idx + 2] as u32;
            pixels[row * width_u + col] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }
    }

    Some(pixels)
}

fn font_bitmap(ch: char) -> [[bool; 5]; 7] {
    let uppercase = ch.to_ascii_uppercase();
    match uppercase {
        'A' => [
            [false, true, true, true, false],
            [true, false, false, false, true],
            [true, false, false, false, true],
            [true, true, true, true, true],
            [true, false, false, false, true],
            [true, false, false, false, true],
            [true, false, false, false, true],
        ],
        'B' => [
            [true, true, true, true, false],
            [true, false, false, false, true],
            [true, true, true, true, false],
            [true, false, false, false, true],
            [true, false, false, false, true],
            [true, true, true, true, false],
            [false, false, false, false, false],
        ],
        'C' => [
            [false, true, true, true, true],
            [true, false, false, false, false],
            [true, false, false, false, false],
            [true, false, false, false, false],
            [true, false, false, false, false],
            [false, true, true, true, true],
            [false, false, false, false, false],
        ],
        _ => [[false; 5]; 7],
    }
}
