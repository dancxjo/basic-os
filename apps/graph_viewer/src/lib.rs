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
const ICON_HOME: &[u8] = include_bytes!("../../../widgets/button/icons/home.bmp");

pub struct GraphViewerApp {
    window: WindowHandle,
    nodes: BTreeMap<Uuid, NodeInfo>,
    edges: Vec<EdgeInfo>,
    watch_id: Option<userland::watch::WatchId>,
    width: u64,
    height: u64,
    scroll_y: i64,
    scrollbar_id: Option<Uuid>,
}

struct NodeInfo {
    // x, y removed as we layout dynamically
    label: String,
    icon_name: Option<String>,
    kind: String,
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
            is_place_root: false,
            place_id: None,
            mode_index: Some(0), // F1
            window_rect: None,
            gap: None,
            flex_direction: None,
            justify_content: None,
            align_items: None,
        };
        let window = ctx.create_window_with(window_fields);

        // Create scrollbar
        let scrollbar_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"graph_viewer_scrollbar");
        let mut sb_fields = graph::map();
        sb_fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        sb_fields.insert(
            canon::cc('W', 'K'),
            Value::Text("scrollbar_thumb".to_string()),
        );
        sb_fields.insert(canon::X, Value::I64((width - 20) as i64));
        sb_fields.insert(canon::Y, Value::I64(0));
        sb_fields.insert(canon::WIDTH, Value::U64(20));
        sb_fields.insert(canon::HEIGHT, Value::U64(height));
        sb_fields.insert(canon::PARENT, Value::Uuid(window.window_id()));
        sb_fields.insert(canon::VISIBLE, Value::Bool(true));
        sb_fields.insert(canon::VIEWPORT_HEIGHT, Value::I64(height as i64));
        sb_fields.insert(canon::CONTENT_HEIGHT, Value::I64(0));
        sb_fields.insert(canon::SCROLL_Y, Value::I64(0));
        graph::fiat(Some(scrollbar_id), canon::WIDGET, sb_fields);

        // Grant access to widget host
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        graph::grant_capability(widget_host_bundle, scrollbar_id, "CAN_READ");
        graph::grant_capability(widget_host_bundle, scrollbar_id, "CAN_WRITE"); // Scrollbar updates itself

        let mut app = GraphViewerApp {
            window: window.clone(),
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            watch_id: None,
            width,
            height,
            scroll_y: 0,
            scrollbar_id: Some(scrollbar_id),
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
                // Check if it's our scrollbar
                if Some(thing.id) == self.scrollbar_id {
                    if let Some(Value::I64(y)) = thing.fields.get(&canon::SCROLL_Y) {
                        self.scroll_y = *y;
                        self.redraw(ctx);
                    }
                    return;
                }

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

                let kind = format!("{:?}", thing.kind);

                self.nodes.insert(
                    thing.id,
                    NodeInfo {
                        label,
                        icon_name,
                        kind,
                    },
                );

                // Update scrollbar content height
                if let Some(sb_id) = self.scrollbar_id {
                    let row_height = 30;
                    let content_height = self.nodes.len() as i64 * row_height;
                    let mut updates = graph::map();
                    updates.insert(canon::CONTENT_HEIGHT, Value::I64(content_height));
                    graph::fiat(Some(sb_id), canon::WIDGET, updates);
                }

                self.redraw(ctx);
            }
            AppEvent::Edge { edge, .. } => {
                self.edges.push(EdgeInfo {
                    src: edge.src,
                    dst: edge.dst,
                });
                // self.redraw(ctx); // Edges not shown in list view
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {}
}

impl GraphViewerApp {
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

        // Draw list items
        let row_height = 30;
        let header_height = 30;
        let list_y_offset = header_height;

        let mut y = list_y_offset - self.scroll_y as i32;

        for (id, node) in &self.nodes {
            if y + row_height > list_y_offset && y < height as i32 {
                // Draw row background (alternating slightly for visibility)
                // let bg = if (y / row_height) % 2 == 0 { 0xFFEEEEEE } else { 0xFFDDDDDD };
                // draw_rect(&mut pixels, width, 0, y, width as i32, row_height, bg);

                // Draw Icon
                draw_bmp_bytes(&mut pixels, width, 10, y + 3, ICON_HOME);

                // Draw Label
                draw_text(&mut pixels, width, 50, y + 7, &node.label, 0x000000);

                // Draw Kind
                draw_text(&mut pixels, width, 300, y + 7, &node.kind, 0x444444);

                // Draw ID
                draw_text(&mut pixels, width, 500, y + 7, &id.to_string(), 0x888888);
            }
            y += row_height;
        }

        // Draw Header (Last, to clip items)
        draw_rect(
            &mut pixels,
            width,
            0,
            0,
            width as i32,
            header_height,
            0xCCCCCC,
        );
        draw_text(&mut pixels, width, 10, 7, "Icon", 0x000000);
        draw_text(&mut pixels, width, 50, 7, "Name", 0x000000);
        draw_text(&mut pixels, width, 300, 7, "Kind", 0x000000);
        draw_text(&mut pixels, width, 500, 7, "ID", 0x000000);

        // Debug: Node count
        let count_str = format!("Count: {}", self.nodes.len());
        draw_text(&mut pixels, width, 800, 7, &count_str, 0xFF0000);

        // Encode BMP
        let bmp_data = encode_bmp(width, height, &pixels);
        ctx.draw_bitmap(&self.window, &bmp_data);
    }
}

fn draw_rect(pixels: &mut [u32], width: usize, x: i32, y: i32, w: i32, h: i32, color: u32) {
    for row in 0..h {
        let dst_y = y + row;
        if dst_y < 0 || dst_y >= (pixels.len() / width) as i32 {
            continue;
        }
        for col in 0..w {
            let dst_x = x + col;
            if dst_x < 0 || dst_x >= width as i32 {
                continue;
            }
            pixels[dst_y as usize * width + dst_x as usize] = color;
        }
    }
}

fn draw_text(pixels: &mut [u32], width: usize, x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for c in text.chars() {
        if let Some(glyph) = unifont::get_glyph(c) {
            let glyph_width = glyph.get_width() as i32;
            for row in 0..16 {
                let dst_y = y + row;
                if dst_y < 0 || dst_y >= (pixels.len() / width) as i32 {
                    continue;
                }
                for col in 0..glyph_width {
                    let dst_x = cx + col;
                    if dst_x < 0 || dst_x >= width as i32 {
                        continue;
                    }
                    if glyph.get_pixel(col as usize, row as usize) {
                        pixels[dst_y as usize * width + dst_x as usize] = color;
                    }
                }
            }
            cx += glyph_width;
        } else {
            cx += 8;
        }
    }
}

fn draw_bmp_bytes(pixels: &mut [u32], width: usize, x: i32, y: i32, bmp: &[u8]) {
    if bmp.len() < 54 {
        return;
    }
    let w = i32::from_le_bytes(bmp[18..22].try_into().unwrap_or([0; 4])).abs();
    let h = i32::from_le_bytes(bmp[22..26].try_into().unwrap_or([0; 4])).abs();
    let offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap_or([0; 4])) as usize;
    let top_down = i32::from_le_bytes(bmp[22..26].try_into().unwrap_or([0; 4])) < 0;

    if w <= 0 || h <= 0 || offset >= bmp.len() {
        return;
    }

    for row in 0..h {
        for col in 0..w {
            let src_row = if top_down { row } else { h - 1 - row };
            let src_idx = offset + (src_row as usize * w as usize + col as usize) * 4;

            if src_idx + 4 > bmp.len() {
                continue;
            }

            let b = bmp[src_idx];
            let g = bmp[src_idx + 1];
            let r = bmp[src_idx + 2];
            let a = bmp[src_idx + 3];

            if a == 0 {
                continue;
            }

            let dst_x = x + col;
            let dst_y = y + row;

            if dst_x < 0
                || dst_x >= width as i32
                || dst_y < 0
                || dst_y >= (pixels.len() / width) as i32
            {
                continue;
            }

            // Simple alpha blending over existing pixel
            // Existing pixel is 0x00RRGGBB
            let dst_idx = dst_y as usize * width + dst_x as usize;
            let dst_pixel = pixels[dst_idx];
            let dst_r = ((dst_pixel >> 16) & 0xFF) as u8;
            let dst_g = ((dst_pixel >> 8) & 0xFF) as u8;
            let dst_b = (dst_pixel & 0xFF) as u8;

            let inv_a = 255 - a;
            let out_r = ((r as u16 * a as u16 + dst_r as u16 * inv_a as u16) / 255) as u8;
            let out_g = ((g as u16 * a as u16 + dst_g as u16 * inv_a as u16) / 255) as u8;
            let out_b = ((b as u16 * a as u16 + dst_b as u16 * inv_a as u16) / 255) as u8;

            pixels[dst_idx] = ((out_r as u32) << 16) | ((out_g as u32) << 8) | (out_b as u32);
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
