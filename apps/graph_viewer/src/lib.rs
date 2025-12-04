#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter};
use uuid::Uuid;

const CLOUDS_BMP: &[u8] = include_bytes!("../../../clouds.bmp");

pub struct GraphViewerApp {
    window: WindowHandle,
    nodes: BTreeMap<Uuid, NodeInfo>,
    edges: Vec<EdgeInfo>,
    watch_id: Option<userland::watch::WatchId>,
    width: u64,
    height: u64,
}

struct NodeInfo {
    x: i64,
    y: i64,
    widget_id: Option<Uuid>,
    label: String,
    icon_name: Option<String>,
}

struct EdgeInfo {
    src: Uuid,
    dst: Uuid,
}

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let width = 1024;
        let height = 768;
        let mut window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Graph Viewer".to_string(),
            x: 0,
            y: 0,
            width,
            height,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            mode_index: Some(0), // F1
            window_rect: None,
        };
        let window = ctx.create_window_with(window_fields);

        let mut app = GraphViewerApp {
            window: window.clone(),
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            watch_id: None,
            width,
            height,
        };

        // Watch everything
        app.watch_id = Some(ctx.watch_graph(ThingFilter {
            kind: None,
            id: None,
        }));

        app.redraw(ctx);
        app
    }

    fn on_event(&mut self, ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                // Update node
                let label = thing
                    .fields
                    .get(&canon::NAME)
                    .or_else(|| thing.fields.get(&canon::TITLE))
                    .or_else(|| thing.fields.get(&canon::TEXT))
                    .and_then(|v| match v {
                        Value::Text(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("{:?}", thing.kind));

                let icon_name = thing.fields.get(&canon::ICON_NAME).and_then(|v| match v {
                    Value::Text(s) => Some(s.clone()),
                    _ => None,
                });

                if let Some(node) = self.nodes.get_mut(&thing.id) {
                    node.label = label.clone();
                    node.icon_name = icon_name.clone();
                    // Update widget label if exists
                    if let Some(widget_id) = node.widget_id {
                        let mut updates = graph::map();
                        updates.insert(canon::TEXT, Value::Text(node.label.clone()));
                        if let Some(icon) = &node.icon_name {
                            updates.insert(canon::ICON_NAME, Value::Text(icon.clone()));
                        }
                        graph::fiat(Some(widget_id), canon::WIDGET, updates);
                    }
                } else {
                    // New node
                    // Simple grid layout
                    let idx = self.nodes.len() as i64;
                    let cols = 10;
                    let spacing_x = 100;
                    let spacing_y = 80;
                    let margin_x = 50;
                    let margin_y = 50;

                    let x = (idx % cols) * spacing_x + margin_x;
                    let y = (idx / cols) * spacing_y + margin_y;

                    let widget_id =
                        self.create_node_widget(ctx, x, y, &label, icon_name.as_deref());

                    self.nodes.insert(
                        thing.id,
                        NodeInfo {
                            x,
                            y,
                            widget_id: Some(widget_id),
                            label,
                            icon_name,
                        },
                    );
                    self.redraw(ctx);
                }
            }
            AppEvent::Edge { edge, .. } => {
                self.edges.push(EdgeInfo {
                    src: edge.src,
                    dst: edge.dst,
                });
                self.redraw(ctx);
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {}
}

impl GraphViewerApp {
    fn create_node_widget(
        &self,
        _ctx: &mut AppContext<'_>,
        x: i64,
        y: i64,
        label: &str,
        icon_name: Option<&str>,
    ) -> Uuid {
        let widget_id = userland::simple_uuid(alloc::format!("node_{}_{}", x, y).as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        fields.insert(canon::cc('W', 'K'), Value::Text("button".to_string()));
        fields.insert(canon::TEXT, Value::Text(label.to_string()));
        if let Some(icon) = icon_name {
            fields.insert(canon::ICON_NAME, Value::Text(icon.to_string()));
        }
        fields.insert(canon::X, Value::I64(x));
        fields.insert(canon::Y, Value::I64(y));
        fields.insert(canon::WIDTH, Value::U64(80));
        fields.insert(canon::HEIGHT, Value::U64(30));
        fields.insert(canon::PARENT, Value::Uuid(self.window.window_id()));
        fields.insert(canon::VISIBLE, Value::Bool(true));

        graph::fiat(Some(widget_id), canon::WIDGET, fields);

        // Grant access to widget host
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        graph::grant_capability(widget_host_bundle, widget_id, "CAN_READ");

        widget_id
    }

    fn redraw(&self, ctx: &mut AppContext<'_>) {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut pixels = vec![0u32; width * height];

        // Parse CLOUDS_BMP and tile it
        if CLOUDS_BMP.len() > 54 {
            let w =
                i32::from_le_bytes(CLOUDS_BMP[18..22].try_into().unwrap_or([0; 4])).abs() as usize;
            let h =
                i32::from_le_bytes(CLOUDS_BMP[22..26].try_into().unwrap_or([0; 4])).abs() as usize;
            let offset =
                u32::from_le_bytes(CLOUDS_BMP[10..14].try_into().unwrap_or([0; 4])) as usize;

            if w > 0 && h > 0 && offset < CLOUDS_BMP.len() {
                for y in 0..height {
                    for x in 0..width {
                        let src_x = x % w;
                        let src_y = y % h;
                        // Assuming bottom-up BMP
                        let row = h - 1 - src_y;
                        let stride = (w * 3 + 3) & !3;
                        let idx = offset + row * stride + src_x * 3;
                        if idx + 2 < CLOUDS_BMP.len() {
                            let b = CLOUDS_BMP[idx];
                            let g = CLOUDS_BMP[idx + 1];
                            let r = CLOUDS_BMP[idx + 2];
                            pixels[y * width + x] =
                                ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        }
                    }
                }
            }
        } else {
            // Fallback
            for p in pixels.iter_mut() {
                *p = 0xFF87CEEB;
            }
        }

        // Draw edges
        for edge in &self.edges {
            if let (Some(src), Some(dst)) = (self.nodes.get(&edge.src), self.nodes.get(&edge.dst)) {
                // Draw line from center of nodes
                draw_line(
                    &mut pixels,
                    width,
                    src.x + 40,
                    src.y + 15,
                    dst.x + 40,
                    dst.y + 15,
                    0xFF000000,
                );
            }
        }

        // Encode BMP
        let bmp_data = encode_bmp(width, height, &pixels);
        ctx.draw_bitmap(&self.window, &bmp_data);
    }
}

fn draw_line(pixels: &mut [u32], width: usize, x0: i64, y0: i64, x1: i64, y1: i64, color: u32) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < width as i64 && y0 >= 0 && y0 < (pixels.len() / width) as i64 {
            pixels[y0 as usize * width + x0 as usize] = color;
        }
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

fn encode_bmp(width: usize, height: usize, pixels: &[u32]) -> Vec<u8> {
    let mut data = Vec::new();
    // Header
    data.extend_from_slice(b"BM");
    let file_size = 54 + width * height * 3;
    data.extend_from_slice(&(file_size as u32).to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(&54u32.to_le_bytes());
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(width as i32).to_le_bytes());

    // Negative height for top-down
    let h_neg = -(height as i32);
    data.extend_from_slice(&h_neg.to_le_bytes());

    data.extend_from_slice(&1u16.to_le_bytes()); // Planes
    data.extend_from_slice(&24u16.to_le_bytes()); // BPP
    data.extend_from_slice(&0u32.to_le_bytes()); // Compression
    data.extend_from_slice(&0u32.to_le_bytes()); // Image size
    data.extend_from_slice(&0u32.to_le_bytes()); // X ppm
    data.extend_from_slice(&0u32.to_le_bytes()); // Y ppm
    data.extend_from_slice(&0u32.to_le_bytes()); // Colors used
    data.extend_from_slice(&0u32.to_le_bytes()); // Colors important

    let padding = (4 - (width * 3) % 4) % 4;

    for i in 0..height {
        for j in 0..width {
            let pixel = pixels[i * width + j];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            data.push(b);
            data.push(g);
            data.push(r);
        }
        for _ in 0..padding {
            data.push(0);
        }
    }

    data
}
