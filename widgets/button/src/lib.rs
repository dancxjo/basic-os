#![no_std]

extern crate alloc;

use alloc::string::String;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;
use userland::graph::{set_props, GraphPropsRequest};

use userland::widget_abi::*;
use userland::{canon, Symbol, Value};

// Simple IconPath struct to hold SVG path data
#[derive(Clone, Debug)]
pub struct IconPath {
    pub data: alloc::string::String,
    pub width: u32,
    pub height: u32,
}

const ICON_PADDING: i32 = 4;
const LABEL_SIDE_PADDING: i32 = 8;
const MIN_BUTTON_WIDTH: i32 = 24;
const DEFAULT_GLYPH_WIDTH: i32 = 8;
const TEXT_HEIGHT: i32 = 16;
const BIND_INDEX_SYM: Symbol = canon::canon(b'I', b'D', b'X');

pub struct ButtonWidget;

#[derive(Clone, Debug)]
pub struct State {
    pub label: String,
    pub target: String,
    pub pressed: bool,
    pub hovered: bool,
    pub focused: bool,
    pub icon_name: Option<String>,
    pub icon: Option<IconPath>,
    pub show_label: bool,
    pub bind_node: Option<userland::uuid::Uuid>,
    pub bind_index: Option<i64>,
}

impl WidgetAbi for ButtonWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        // Read properties from graph
        let mut label = String::from("Btn");
        let mut target = String::new();
        let mut icon_name = String::new();
        let mut show_label = true;
        let mut bind_node = None;
        let mut bind_index = None;

        // Define a local symbol for SHOW_LABEL until it's standardized
        let show_label_sym = canon::canon(b'S', b'H', b'L');

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![
                canon::TEXT,
                canon::TARGET,
                canon::ICON_NAME,
                show_label_sym,
                canon::BINDS,
                BIND_INDEX_SYM,
            ],
        };

        if let Some(props) = get_props(req) {
            if let Some(Value::Text(t)) = props.get(&canon::TEXT) {
                label = t.clone();
            }
            if let Some(Value::Text(t)) = props.get(&canon::TARGET) {
                target = t.clone();
            }
            if let Some(Value::Text(t)) = props.get(&canon::ICON_NAME) {
                icon_name = t.clone();
            }
            if let Some(Value::Bool(b)) = props.get(&show_label_sym) {
                show_label = *b;
            }
            if let Some(Value::Uuid(b)) = props.get(&canon::BINDS) {
                bind_node = Some(*b);
            }
            if let Some(Value::I64(idx)) = props.get(&BIND_INDEX_SYM) {
                bind_index = Some(*idx);
            }
        }

        let icon_name = if icon_name.is_empty() {
            None
        } else {
            Some(icon_name)
        };

        let icon = icon_name.as_ref().and_then(|name| {
            userland::println!("ButtonWidget: Loading icon '{}'", name);
            load_icon(name)
        });

        if icon.is_none() && icon_name.is_some() {
            userland::println!("ButtonWidget: Icon load failed for {:?}", icon_name);
        }

        State {
            label,
            target,
            pressed: false,
            hovered: false,
            focused: false,
            icon_name,
            icon,
            show_label,
            bind_node,
            bind_index,
        }
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        // Style constants (Mac OS Classic-ish / Windows 3.1)
        let bg_color_normal: u32 = 0xFF_C0_C0_C0; // Standard Windows gray
        let bg_color_pressed: u32 = 0xFF_A0_A0_A0; // Darker gray
        let border_highlight: u32 = 0xFF_FF_FF_FF; // White
        let border_shadow: u32 = 0xFF_40_40_40; // Dark Gray
        let border_black: u32 = 0xFF_00_00_00; // Black
        let text_color: u32 = 0xFF_00_00_00; // Black

        let bg_color = if state.pressed {
            bg_color_pressed
        } else if state.hovered {
            0xFF_E0_E0_E0 // Lighter gray for hover
        } else {
            bg_color_normal
        };

        // Draw button background
        for y in 0..rect.height {
            for x in 0..rect.width {
                let offset = ((rect.y + y as i32) as usize * rect.width as usize
                    + (rect.x + x as i32) as usize)
                    * 4;
                if offset + 4 <= fb.len() {
                    fb[offset] = (bg_color >> 16) as u8;
                    fb[offset + 1] = (bg_color >> 8) as u8;
                    fb[offset + 2] = bg_color as u8;
                    fb[offset + 3] = (bg_color >> 24) as u8;
                }
            }
        }

        // Draw 3D Bevel
        // Outer Black Border
        draw_rect_outline(
            fb,
            rect,
            0,
            0,
            rect.width as i32,
            rect.height as i32,
            border_black,
        );

        if state.pressed {
            // Pressed: Shadow Top/Left, Highlight Bottom/Right (Inset)
            draw_line_h(fb, rect, 1, 1, rect.width as i32 - 2, border_shadow); // Top
            draw_line_v(fb, rect, 1, 1, rect.height as i32 - 2, border_shadow); // Left
                                                                                // No highlight on bottom/right for pressed usually, or just flat.
                                                                                // Let's do simple inset.
        } else {
            // Normal: Highlight Top/Left, Shadow Bottom/Right (Outset)
            draw_line_h(fb, rect, 1, 1, rect.width as i32 - 2, border_highlight); // Top
            draw_line_v(fb, rect, 1, 1, rect.height as i32 - 2, border_highlight); // Left

            draw_line_h(
                fb,
                rect,
                1,
                rect.height as i32 - 2,
                rect.width as i32 - 2,
                border_shadow,
            ); // Bottom
            draw_line_v(
                fb,
                rect,
                rect.width as i32 - 2,
                1,
                rect.height as i32 - 2,
                border_shadow,
            ); // Right
        }

        // Draw icon
        let content_offset = if state.pressed { 1 } else { 0 };
        let icon = &state.icon;

        if let Some(icon) = icon {
            userland::println!("ButtonWidget: Drawing icon, data len={}", icon.data.len());
            let x = if state.show_label {
                ICON_PADDING
            } else {
                (rect.width as i32 - icon.width as i32) / 2
            };
            // Use text_color for icon to match label/context
            draw_svg(
                fb,
                rect,
                x + content_offset,
                (rect.height as i32 - icon.height as i32) / 2 + content_offset,
                icon,
                text_color,
            );
        }

        // Draw label if enabled
        if state.show_label {
            // Calculate text width for centering
            let text_width = measure_text_width(&state.label);

            let mut x = if let Some(icon) = icon {
                ICON_PADDING + icon.width as i32 + ICON_PADDING
            } else {
                (rect.width as i32 - text_width) / 2
            };
            let y = (rect.height as i32 - TEXT_HEIGHT) / 2;
            for c in state.label.chars() {
                draw_char(
                    fb,
                    rect,
                    x + content_offset,
                    y + content_offset,
                    c,
                    text_color,
                );
                if let Some(glyph) = unifont::get_glyph(c) {
                    x += glyph.get_width() as i32;
                } else {
                    x += 8;
                }
            }
        }

        // Draw focus ring
        if state.focused {
            let focus_color = 0xFF_00_00_00;
            let inset = 3;
            let x = inset;
            let y = inset;
            let w = rect.width as i32 - inset * 2;
            let h = rect.height as i32 - inset * 2;

            // Simple dotted line (every other pixel)
            for i in 0..w {
                if i % 2 == 0 {
                    draw_pixel(fb, rect, x + i, y, focus_color);
                    draw_pixel(fb, rect, x + i, y + h - 1, focus_color);
                }
            }
            for i in 0..h {
                if i % 2 == 0 {
                    draw_pixel(fb, rect, x, y + i, focus_color);
                    draw_pixel(fb, rect, x + w - 1, y + i, focus_color);
                }
            }
        }
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { .. }) => {
                state.pressed = true;
            }
            WidgetEvent::Input(InputEvent::MouseUp { .. }) => {
                if state.pressed {
                    activate(state);
                    state.pressed = false;
                }
            }
            WidgetEvent::Input(InputEvent::KeyDown { key }) => {
                let sym = Symbol::new(key);
                if sym == canon::cc('E', 'N') || sym == canon::cc(' ', ' ') {
                    activate(state);
                }
            }
            WidgetEvent::Input(InputEvent::MouseMove { .. }) => {
                // Simple hover detection: if we get move events, we are hovered.
                // Ideally we'd get MouseLeave, but for now this is a start.
                if !state.hovered {
                    state.hovered = true;
                }
            }
            _ => {}
        }
    }

    fn teardown(_state: Self::State) {}
}

impl ButtonWidget {
    pub fn intrinsic_size(state: &State, base_height: u32) -> (u32, u32) {
        let text_width = if state.show_label {
            measure_text_width(&state.label)
        } else {
            0
        };

        let icon = &state.icon;

        let icon_width = icon.as_ref().map(|i| i.width as i32).unwrap_or(0);
        let icon_height = icon.as_ref().map(|i| i.height as i32).unwrap_or(0);
        let label_height = if state.show_label { TEXT_HEIGHT } else { 0 };
        let content_height = icon_height.max(label_height);

        let width = if icon.is_some() {
            if state.show_label {
                ICON_PADDING + icon_width + ICON_PADDING + text_width + ICON_PADDING
            } else {
                icon_width + ICON_PADDING as i32 * 2
            }
        } else if state.show_label {
            text_width + LABEL_SIDE_PADDING as i32 * 2
        } else {
            MIN_BUTTON_WIDTH as i32
        };

        let height_needed = content_height + ICON_PADDING as i32 * 2;
        let height = base_height.max(height_needed.max(1) as u32);

        (width.max(MIN_BUTTON_WIDTH as i32) as u32, height)
    }
}

fn draw_char(fb: &mut [u8], rect: Rect, x: i32, y: i32, c: char, color: u32) {
    if let Some(glyph) = unifont::get_glyph(c) {
        let glyph_width = glyph.get_width() as i32;
        for row in 0..16 {
            let dst_y = rect.y + y + row;
            if dst_y < rect.y || dst_y >= rect.y + rect.height as i32 {
                continue;
            }
            for col in 0..glyph_width {
                let dst_x = rect.x + x + col;
                if dst_x < rect.x || dst_x >= rect.x + rect.width as i32 {
                    continue;
                }
                if glyph.get_pixel(col as usize, row as usize) {
                    let offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
                    if offset + 4 <= fb.len() {
                        fb[offset] = (color >> 16) as u8;
                        fb[offset + 1] = (color >> 8) as u8;
                        fb[offset + 2] = color as u8;
                        fb[offset + 3] = (color >> 24) as u8;
                    }
                }
            }
        }
    }
}

fn measure_text_width(text: &str) -> i32 {
    text.chars()
        .map(|c| {
            if let Some(glyph) = unifont::get_glyph(c) {
                glyph.get_width() as i32
            } else {
                DEFAULT_GLYPH_WIDTH
            }
        })
        .sum()
}

fn draw_svg(
    fb: &mut [u8],
    rect: Rect,
    start_x: i32,
    start_y: i32,
    icon: &IconPath,
    color_val: u32,
) {
    let path_str = &icon.data;

    let cst = match svg_path_cst::svg_path_cst(path_str.as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut pen_x: f32 = 0.0;
    let mut pen_y: f32 = 0.0;
    let mut start_subpath_x: f32 = 0.0;
    let mut start_subpath_y: f32 = 0.0;

    for node in cst {
        match node {
            svg_path_cst::SVGPathCSTNode::Segment(segment) => {
                let args = segment.args;
                match segment.command {
                    svg_path_cst::SVGPathCommand::MovetoUpper => {
                        if args.len() >= 2 {
                            pen_x = args[0] as f32;
                            pen_y = args[1] as f32;
                            start_subpath_x = pen_x;
                            start_subpath_y = pen_y;
                        }
                    }
                    svg_path_cst::SVGPathCommand::MovetoLower => {
                        if args.len() >= 2 {
                            pen_x += args[0] as f32;
                            pen_y += args[1] as f32;
                            start_subpath_x = pen_x;
                            start_subpath_y = pen_y;
                        }
                    }
                    svg_path_cst::SVGPathCommand::LinetoUpper => {
                        let mut i = 0;
                        while i + 1 < args.len() {
                            let nx = args[i] as f32;
                            let ny = args[i + 1] as f32;
                            draw_line_segment(
                                fb,
                                rect,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + pen_y) as i32,
                                (start_x as f32 + nx) as i32,
                                (start_y as f32 + ny) as i32,
                                color_val,
                            );
                            pen_x = nx;
                            pen_y = ny;
                            i += 2;
                        }
                    }
                    svg_path_cst::SVGPathCommand::LinetoLower => {
                        let mut i = 0;
                        while i + 1 < args.len() {
                            let nx = pen_x + args[i] as f32;
                            let ny = pen_y + args[i + 1] as f32;
                            draw_line_segment(
                                fb,
                                rect,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + pen_y) as i32,
                                (start_x as f32 + nx) as i32,
                                (start_y as f32 + ny) as i32,
                                color_val,
                            );
                            pen_x = nx;
                            pen_y = ny;
                            i += 2;
                        }
                    }
                    svg_path_cst::SVGPathCommand::HorizontalUpper => {
                        for i in 0..args.len() {
                            let nx = args[i] as f32;
                            draw_line_segment(
                                fb,
                                rect,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + pen_y) as i32,
                                (start_x as f32 + nx) as i32,
                                (start_y as f32 + pen_y) as i32,
                                color_val,
                            );
                            pen_x = nx;
                        }
                    }
                    svg_path_cst::SVGPathCommand::VerticalUpper => {
                        for i in 0..args.len() {
                            let ny = args[i] as f32;
                            draw_line_segment(
                                fb,
                                rect,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + pen_y) as i32,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + ny) as i32,
                                color_val,
                            );
                            pen_y = ny;
                        }
                    }
                    svg_path_cst::SVGPathCommand::ClosepathUpper
                    | svg_path_cst::SVGPathCommand::ClosepathLower => {
                        draw_line_segment(
                            fb,
                            rect,
                            (start_x as f32 + pen_x) as i32,
                            (start_y as f32 + pen_y) as i32,
                            (start_x as f32 + start_subpath_x) as i32,
                            (start_y as f32 + start_subpath_y) as i32,
                            color_val,
                        );
                        pen_x = start_subpath_x;
                        pen_y = start_subpath_y;
                    }
                    svg_path_cst::SVGPathCommand::ArcUpper => {
                        let mut i = 0;
                        while i + 6 < args.len() {
                            let nx = args[i + 5] as f32;
                            let ny = args[i + 6] as f32;
                            draw_line_segment(
                                fb,
                                rect,
                                (start_x as f32 + pen_x) as i32,
                                (start_y as f32 + pen_y) as i32,
                                (start_x as f32 + nx) as i32,
                                (start_y as f32 + ny) as i32,
                                color_val,
                            );
                            pen_x = nx;
                            pen_y = ny;
                            i += 7;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

// Simple Bresenham Line
fn draw_line_segment(fb: &mut [u8], rect: Rect, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        draw_pixel(fb, rect, x, y, color);
        if x == x1 && y == y1 {
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

pub fn load_icon(name: &str) -> Option<IconPath> {
    // Try to load from graph first
    let things = userland::graph::find_by_kind("FIL");
    for thing in things {
        if let Some(Value::Text(icon_name)) = thing.fields.get(&canon::ICON_NAME) {
            if icon_name == name {
                if let Some(Value::Bytes(bytes)) = thing.fields.get(&canon::BYTES) {
                    if let Ok(svg_str) = core::str::from_utf8(bytes) {
                        // Extract path data from SVG XML
                        if let Some(path_data) = extract_svg_path(svg_str) {
                            return Some(IconPath {
                                data: path_data,
                                width: 24,
                                height: 24,
                            });
                        }
                    }
                }
            }
        }
    }

    // Fallback to hardcoded icons for essential UI elements
    let (data, width, height) = match name {
        "close" => ("M 18 6 L 6 18 M 6 6 L 18 18", 24, 24),
        "menu" => ("M 3 12 L 21 12 M 3 6 L 21 6 M 3 18 L 21 18", 24, 24),
        "arrow-back" => ("M 19 12 L 5 12 M 12 19 L 5 12 L 12 5", 24, 24),
        _ => return None,
    };

    Some(IconPath {
        data: alloc::string::String::from(data),
        width,
        height,
    })
}

/// Extract the `d` attribute from an SVG path element
fn extract_svg_path(svg: &str) -> Option<alloc::string::String> {
    // Simple extraction: find d="..." attribute
    if let Some(d_idx) = svg.find(" d=\"") {
        let start = d_idx + 4;
        if let Some(end) = svg[start..].find('"') {
            return Some(alloc::string::String::from(&svg[start..start + end]));
        }
    }
    None
}

include!("helpers.rs");

fn activate(state: &State) {
    if let Some(bind_node) = state.bind_node {
        if let Some(idx) = state.bind_index {
            let mut updates = userland::map();
            updates.insert(canon::SELECTED_INDEX, Value::I64(idx));
            set_props(GraphPropsRequest {
                node: bind_node,
                props: updates,
            });
        }
    }

    if !state.target.is_empty() {
        // Emit LaunchRequest
        let mut fields = userland::map();
        fields.insert(canon::PACKAGE, Value::Text(state.target.clone()));
        fields.insert(canon::NAME, Value::Text(state.label.clone()));
        userland::fiat(None, canon::LAUNCH_REQUEST, fields);
    }
}
