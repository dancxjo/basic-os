#![no_std]

extern crate alloc;

use alloc::string::String;
use thing_abi::GraphPropsGetRequest;
use userland::graph::get_props;

use core::convert::TryInto;
use userland::widget_abi::*;
use userland::{canon, Value};

#[derive(Clone, Debug)]
pub struct Icon {
    data: &'static [u8],
    width: i32,
    height: i32,
    data_offset: usize,
    top_down: bool,
}

struct IconDef {
    name: &'static str,
    data: &'static [u8],
}

const ICON_DEFS: &[IconDef] = &[
    IconDef {
        name: "close",
        data: ICON_CLOSE,
    },
    IconDef {
        name: "settings",
        data: ICON_SETTINGS,
    },
    IconDef {
        name: "home",
        data: ICON_HOME,
    },
    IconDef {
        name: "arrow-back",
        data: ICON_ARROW_BACK,
    },
    IconDef {
        name: "menu",
        data: ICON_MENU,
    },
];

pub const ICON_CLOSE: &[u8] = include_bytes!("../icons/close.bmp");
pub const ICON_SETTINGS: &[u8] = include_bytes!("../icons/settings.bmp");
pub const ICON_HOME: &[u8] = include_bytes!("../icons/home.bmp");
pub const ICON_ARROW_BACK: &[u8] = include_bytes!("../icons/arrow-back.bmp");
pub const ICON_MENU: &[u8] = include_bytes!("../icons/menu.bmp");

const ICON_PADDING: i32 = 4;
const LABEL_SIDE_PADDING: i32 = 8;
const MIN_BUTTON_WIDTH: i32 = 24;
const DEFAULT_GLYPH_WIDTH: i32 = 8;
const TEXT_HEIGHT: i32 = 16;

pub struct ButtonWidget;

#[derive(Clone, Debug)]
pub struct State {
    pub label: String,
    pub target: String,
    pub pressed: bool,
    pub hovered: bool,
    pub icon: Option<Icon>,
    pub show_label: bool,
}

impl WidgetAbi for ButtonWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        // Read properties from graph
        let mut label = String::from("Btn");
        let mut target = String::new();
        let mut icon_name = String::new();
        let mut show_label = true;

        // Define a local symbol for SHOW_LABEL until it's standardized
        let show_label_sym = canon::canon(b'S', b'H', b'L');

        let req = GraphPropsGetRequest {
            node: ctx.widget_id,
            keys: alloc::vec![canon::TEXT, canon::TARGET, canon::ICON_NAME, show_label_sym],
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
        }

        let icon = load_icon(icon_name.as_str());

        State {
            label,
            target,
            pressed: false,
            hovered: false,
            icon,
            show_label,
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
        if let Some(icon) = &state.icon {
            let x = if state.show_label {
                ICON_PADDING
            } else {
                (rect.width as i32 - icon.width) / 2
            };
            draw_bmp(
                fb,
                rect,
                x + content_offset,
                (rect.height as i32 - icon.height) / 2 + content_offset,
                icon,
            );
        }

        // Draw label if enabled
        if state.show_label {
            // Calculate text width for centering
            let text_width = measure_text_width(&state.label);

            let mut x = if let Some(icon) = &state.icon {
                ICON_PADDING + icon.width + ICON_PADDING
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
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        match event {
            WidgetEvent::Input(InputEvent::MouseDown { .. }) => {
                state.pressed = true;
            }
            WidgetEvent::Input(InputEvent::MouseUp { .. }) => {
                if state.pressed {
                    state.pressed = false;
                    // Trigger launch
                    if !state.target.is_empty() {
                        // Emit LaunchRequest
                        let mut fields = userland::map();
                        fields.insert(canon::PACKAGE, Value::Text(state.target.clone()));
                        fields.insert(canon::NAME, Value::Text(state.label.clone()));
                        userland::fiat(None, canon::LAUNCH_REQUEST, fields);
                    }
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

        let icon_width = state.icon.as_ref().map(|i| i.width).unwrap_or(0);
        let icon_height = state.icon.as_ref().map(|i| i.height).unwrap_or(0);
        let label_height = if state.show_label { TEXT_HEIGHT } else { 0 };
        let content_height = icon_height.max(label_height);

        let width = if let Some(_) = state.icon {
            if state.show_label {
                ICON_PADDING + icon_width + ICON_PADDING + text_width + ICON_PADDING
            } else {
                icon_width + ICON_PADDING * 2
            }
        } else if state.show_label {
            text_width + LABEL_SIDE_PADDING * 2
        } else {
            MIN_BUTTON_WIDTH
        };

        let height_needed = content_height + ICON_PADDING * 2;
        let height = base_height.max(height_needed.max(1) as u32);

        (width.max(MIN_BUTTON_WIDTH) as u32, height)
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

fn draw_bmp(fb: &mut [u8], rect: Rect, x: i32, y: i32, icon: &Icon) {
    for row in 0..icon.height {
        for col in 0..icon.width {
            let src_row = if icon.top_down {
                row
            } else {
                icon.height - 1 - row
            };
            let src_idx =
                icon.data_offset + (src_row as usize * icon.width as usize + col as usize) * 4;

            if src_idx + 4 > icon.data.len() {
                continue;
            }

            // BMP is BGRA usually
            let b = icon.data[src_idx];
            let g = icon.data[src_idx + 1];
            let r = icon.data[src_idx + 2];
            let a = icon.data[src_idx + 3];

            if a == 0 {
                continue;
            } // Fully transparent

            let dst_x = rect.x + x + col;
            let dst_y = rect.y + y + row;

            if dst_x < rect.x
                || dst_x >= rect.x + rect.width as i32
                || dst_y < rect.y
                || dst_y >= rect.y + rect.height as i32
            {
                continue;
            }

            let dst_offset = (dst_y as usize * rect.width as usize + dst_x as usize) * 4;
            if dst_offset + 4 <= fb.len() {
                // Simple alpha blending
                // dst = src * alpha + dst * (1 - alpha)
                let inv_a = 255 - a;
                let dst_r = fb[dst_offset];
                let dst_g = fb[dst_offset + 1];
                let dst_b = fb[dst_offset + 2];

                fb[dst_offset] = ((r as u16 * a as u16 + dst_r as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 1] =
                    ((g as u16 * a as u16 + dst_g as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 2] =
                    ((b as u16 * a as u16 + dst_b as u16 * inv_a as u16) / 255) as u8;
                fb[dst_offset + 3] = 255; // Opaque alpha for framebuffer
            }
        }
    }
}

fn load_icon(name: &str) -> Option<Icon> {
    let data = ICON_DEFS.iter().find_map(|def| {
        if def.name == name {
            Some(def.data)
        } else {
            None
        }
    })?;

    let (width, height, data_offset, top_down) = parse_bmp_metadata(data)?;

    Some(Icon {
        data,
        width,
        height,
        data_offset,
        top_down,
    })
}

fn parse_bmp_metadata(bmp: &[u8]) -> Option<(i32, i32, usize, bool)> {
    if bmp.len() < 54 {
        return None;
    }
    let width = i32::from_le_bytes(bmp[18..22].try_into().ok()?);
    let height = i32::from_le_bytes(bmp[22..26].try_into().ok()?);
    let data_offset = u32::from_le_bytes(bmp[10..14].try_into().ok()?) as usize;

    if width <= 0 || height == 0 || data_offset >= bmp.len() {
        return None;
    }

    let bpp = u16::from_le_bytes(bmp[28..30].try_into().ok()?);
    if bpp != 32 {
        return None;
    }

    let top_down = height < 0;
    let height_abs = height.abs();
    Some((width, height_abs, data_offset, top_down))
}

include!("helpers.rs");
