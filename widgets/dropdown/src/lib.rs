#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use unifont::get_glyph;
use userland::graph::{
    extract_text, get_props, set_props, GraphPropsGetRequest, GraphPropsRequest,
};
use userland::uuid::Uuid;
use userland::widget_abi::{InputEvent, Rect, WidgetAbi, WidgetContext, WidgetEvent};
use userland::{canon, map, Value};

pub struct DropdownWidget;

#[derive(Clone, Debug)]
struct OptionEntry {
    label: String,
    value: Value,
}

#[derive(Clone, Debug)]
pub struct State {
    widget_id: Uuid,
    options: Vec<OptionEntry>,
    selected_index: usize,
    pressed: bool,
    open: bool,
    label: Option<String>,
    collapsed_height: u32,
    binds: Option<Uuid>,
    hovered_index: Option<usize>,
}

const OPTIONS_SYM: userland::Symbol = canon::canon(b'O', b'P', b'T');
const BG_COLOR: u32 = 0xFFF2_F2_F2;
const BG_PRESSED: u32 = 0xFFE0_E4_EC;
const BORDER_LIGHT: u32 = 0xFFFF_FF_FF;
const BORDER_SHADOW: u32 = 0xFF9C_A3_B2;
const ARROW_BG: u32 = 0xFFD8_DDE8;
const TEXT_COLOR: (u8, u8, u8) = (0x22, 0x22, 0x22);
const PADDING: i32 = 8;
const ARROW_AREA: i32 = 28;
const OPTION_HEIGHT: i32 = 28;
const MENU_BG: u32 = 0xFFFA_FB_FE;
const MENU_SELECTED: u32 = 0xFFE1_E6_F2;
const MENU_HOVER: u32 = 0xFFC8_D6_F0;
const MENU_BORDER: u32 = 0xFF8D_99_AF;

impl WidgetAbi for DropdownWidget {
    type State = State;

    fn init(ctx: &WidgetContext) -> Self::State {
        let mut state = State {
            widget_id: ctx.widget_id,
            options: default_options(),
            selected_index: 0,
            pressed: false,
            open: false,
            label: None,
            collapsed_height: ctx.framebuffer.height.max(1),
            binds: None,
            hovered_index: None,
        };
        refresh_state(&mut state);
        state
    }

    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let mut view = state.clone();
        refresh_state(&mut view);

        let base_height = core::cmp::max(1, core::cmp::min(view.collapsed_height, rect.height));
        let base_height_i = base_height as i32;

        draw_base_control(fb, rect, base_height_i, &view);

        if view.open {
            draw_menu(fb, rect, base_height_i, &view);
        }
    }

    fn handle_event(state: &mut Self::State, event: WidgetEvent) {
        refresh_state(state);

        match event {
            WidgetEvent::Input(InputEvent::MouseDown { y, .. }) => {
                state.pressed = y < state.collapsed_height as i32;
                if state.open {
                    state.hovered_index = option_index_at(state, y);
                } else {
                    open_menu(state);
                }
            }
            WidgetEvent::Input(InputEvent::MouseUp { y, .. }) => {
                if state.open {
                    if let Some(idx) = option_index_at(state, y) {
                        select_index(state, idx);
                    }
                    close_menu(state);
                } else if state.pressed {
                    open_menu(state);
                }
                state.pressed = false;
            }
            WidgetEvent::Input(InputEvent::MouseMove { y, .. }) => {
                if state.open {
                    state.hovered_index = option_index_at(state, y);
                }
            }
            WidgetEvent::Input(InputEvent::KeyDown { key }) => {
                let sym = canon::Symbol::new(key);
                let space = canon::from_char(' ');
                let enter = canon::cc('E', 'N');
                let esc = canon::cc('E', 'S');
                let down = canon::cc('A', 'D');
                let up = canon::cc('A', 'U');
                let home = canon::cc('H', 'M');
                let end = canon::cc('E', 'D');

                if sym == space || sym == enter {
                    if state.open {
                        let idx = state.hovered_index.unwrap_or(state.selected_index);
                        select_index(state, idx);
                        close_menu(state);
                    } else {
                        open_menu(state);
                    }
                } else if sym == down {
                    move_focus(state, 1);
                } else if sym == up {
                    move_focus(state, -1);
                } else if sym == home {
                    if state.open {
                        state.hovered_index = Some(0);
                    } else {
                        select_index(state, 0);
                    }
                } else if sym == end {
                    if let Some(last) = state.options.len().checked_sub(1) {
                        if state.open {
                            state.hovered_index = Some(last);
                        } else {
                            select_index(state, last);
                        }
                    }
                } else if sym == esc {
                    close_menu(state);
                }
            }
            WidgetEvent::Input(InputEvent::KeyUp { .. }) => {
                state.pressed = false;
            }
            _ => {}
        }
    }

    fn teardown(_state: Self::State) {}
}

fn default_options() -> Vec<OptionEntry> {
    alloc::vec![
        option_entry("None"),
        option_entry("Slide"),
        option_entry("Fade"),
        option_entry("Zoom"),
    ]
}

fn option_entry(label: &str) -> OptionEntry {
    OptionEntry {
        label: label.to_string(),
        value: Value::Text(label.to_string()),
    }
}

fn refresh_state(state: &mut State) {
    let req = GraphPropsGetRequest {
        node: state.widget_id,
        keys: alloc::vec![
            OPTIONS_SYM,
            canon::SELECTED_INDEX,
            canon::TEXT,
            canon::HEIGHT,
            canon::BINDS,
        ],
    };

    if let Some(props) = get_props(req) {
        if let Some(Value::List(list)) = props.get(&OPTIONS_SYM) {
            let opts = parse_option_list(list);
            if !opts.is_empty() {
                state.options = opts;
            }
        }
        if let Some(Value::I64(idx)) = props.get(&canon::SELECTED_INDEX) {
            let idx = (*idx).max(0) as usize;
            if !state.options.is_empty() {
                state.selected_index = idx.min(state.options.len() - 1);
            } else {
                state.selected_index = 0;
            }
        }
        if let Some(text) = props.get(&canon::TEXT).and_then(extract_text) {
            state.label = Some(text);
        }
        if let Some(Value::Uuid(node)) = props.get(&canon::BINDS) {
            state.binds = Some(*node);
        }
        if !state.open {
            if let Some(height) = props.get(&canon::HEIGHT).and_then(|v| v.as_u64()) {
                let h = height as u32;
                if h > 0 {
                    state.collapsed_height = h;
                }
            }
        }
    }

    if state.options.is_empty() {
        state.options = default_options();
    }
    if state.selected_index >= state.options.len() {
        state.selected_index = state.options.len().saturating_sub(1);
    }
    if let Some(hover) = state.hovered_index {
        if hover >= state.options.len() {
            state.hovered_index = None;
        }
    }

    if state.open {
        sync_height(state, expanded_height(state));
    }
}

fn parse_option_list(list: &[Value]) -> Vec<OptionEntry> {
    let mut opts = Vec::new();
    for item in list {
        if let Some(label) = value_to_label(item) {
            opts.push(OptionEntry {
                label,
                value: item.clone(),
            });
        }
    }
    opts
}

fn value_to_label(value: &Value) -> Option<String> {
    if let Some(text) = extract_text(value) {
        return Some(text);
    }

    match value {
        Value::Bool(b) => Some(if *b { "true" } else { "false" }.to_string()),
        Value::I64(v) => Some(v.to_string()),
        Value::U64(v) => Some(v.to_string()),
        Value::Symbol(sym) => Some(String::from(*sym)),
        _ => None,
    }
}

fn draw_base_control(fb: &mut [u8], rect: Rect, base_height: i32, state: &State) {
    let bg = if state.pressed || state.open {
        BG_PRESSED
    } else {
        BG_COLOR
    };
    fill_rect(fb, rect, 0, 0, rect.width as i32, base_height, bg);
    draw_rect_outline(
        fb,
        rect,
        0,
        0,
        rect.width as i32,
        base_height,
        BORDER_LIGHT,
        BORDER_SHADOW,
    );

    let arrow_w = ARROW_AREA.min(rect.width as i32);
    let content_w = (rect.width as i32).saturating_sub(arrow_w);
    let arrow_x = rect.width as i32 - arrow_w;

    fill_rect(fb, rect, arrow_x, 0, arrow_w, base_height, ARROW_BG);
    draw_rect_outline(
        fb,
        rect,
        arrow_x,
        0,
        arrow_w,
        base_height,
        BORDER_LIGHT,
        BORDER_SHADOW,
    );
    draw_arrow(fb, rect, arrow_x, base_height, arrow_w);

    let label = current_label(state);
    draw_text_clipped(
        fb,
        rect,
        PADDING,
        ((base_height - 16).max(0)) / 2,
        content_w.saturating_sub(PADDING * 2),
        &label,
        TEXT_COLOR,
    );
}

fn draw_menu(fb: &mut [u8], rect: Rect, base_height: i32, state: &State) {
    let menu_height = rect.height as i32 - base_height;
    if menu_height <= 0 {
        return;
    }

    fill_rect(
        fb,
        rect,
        0,
        base_height,
        rect.width as i32,
        menu_height,
        MENU_BG,
    );

    for (i, opt) in state.options.iter().enumerate() {
        let row_y = base_height + i as i32 * OPTION_HEIGHT;
        if row_y >= rect.height as i32 {
            break;
        }
        let row_h = OPTION_HEIGHT.min(menu_height - (row_y - base_height));
        let is_hover = state.hovered_index == Some(i);
        let is_selected = state.selected_index == i;
        let bg = if is_hover {
            MENU_HOVER
        } else if is_selected {
            MENU_SELECTED
        } else {
            MENU_BG
        };
        fill_rect(fb, rect, 0, row_y, rect.width as i32, row_h, bg);

        draw_text_clipped(
            fb,
            rect,
            PADDING,
            row_y + (OPTION_HEIGHT - 16) / 2,
            rect.width as i32 - PADDING * 2,
            &opt.label,
            TEXT_COLOR,
        );
    }

    draw_rect_outline(
        fb,
        rect,
        0,
        base_height,
        rect.width as i32,
        menu_height,
        MENU_BORDER,
        MENU_BORDER,
    );
}

fn option_index_at(state: &State, y: i32) -> Option<usize> {
    let y_in_menu = y - state.collapsed_height as i32;
    if y_in_menu < 0 {
        return None;
    }
    let idx = y_in_menu / OPTION_HEIGHT;
    if idx < 0 {
        return None;
    }
    let idx = idx as usize;
    if idx < state.options.len() {
        Some(idx)
    } else {
        None
    }
}

fn expanded_height(state: &State) -> u32 {
    let options_h = state.options.len().saturating_mul(OPTION_HEIGHT as usize) as u32;
    state.collapsed_height.saturating_add(options_h)
}

fn sync_height(state: &State, height: u32) {
    let mut props = map();
    props.insert(canon::HEIGHT, Value::U64(height as u64));
    set_props(GraphPropsRequest {
        node: state.widget_id,
        props,
    });
}

fn open_menu(state: &mut State) {
    state.open = true;
    if state.options.is_empty() {
        state.options = default_options();
    }
    if !state.options.is_empty() {
        let idx = state.selected_index.min(state.options.len() - 1);
        state.hovered_index = Some(idx);
    }
    sync_height(state, expanded_height(state));
}

fn close_menu(state: &mut State) {
    state.open = false;
    state.hovered_index = None;
    sync_height(state, state.collapsed_height);
}

fn move_focus(state: &mut State, delta: i32) {
    if state.options.is_empty() {
        return;
    }
    let current = if state.open {
        state.hovered_index.unwrap_or(state.selected_index)
    } else {
        state.selected_index
    } as i32;
    let last = state.options.len() as i32 - 1;
    let next = (current + delta).clamp(0, last) as usize;

    if state.open {
        state.hovered_index = Some(next);
    } else {
        select_index(state, next);
    }
}

fn select_index(state: &mut State, index: usize) {
    if state.options.is_empty() {
        state.options = default_options();
    }
    if state.options.is_empty() {
        return;
    }

    let clamped = index.min(state.options.len() - 1);
    state.selected_index = clamped;
    emit_selection(state);
}

fn emit_selection(state: &State) {
    if state.options.is_empty() {
        return;
    }

    let selected = state.selected_index as i64;
    let label = current_label(state);
    let mut props = map();
    props.insert(canon::SELECTED_INDEX, Value::I64(selected));
    props.insert(canon::TEXT, Value::Text(label.clone()));

    set_props(GraphPropsRequest {
        node: state.widget_id,
        props,
    });

    if let Some(binds) = state.binds {
        let mut bound = map();
        bound.insert(canon::SELECTED_INDEX, Value::I64(selected));
        bound.insert(canon::TEXT, Value::Text(label));
        bound.insert(canon::ITEM_VALUE, current_value(state));
        set_props(GraphPropsRequest {
            node: binds,
            props: bound,
        });
    }
}

fn current_label(state: &State) -> String {
    if let Some(opt) = state.options.get(state.selected_index) {
        return opt.label.clone();
    }
    if let Some(label) = &state.label {
        return label.clone();
    }
    "Select".to_string()
}

fn current_value(state: &State) -> Value {
    if let Some(opt) = state.options.get(state.selected_index) {
        return opt.value.clone();
    }
    Value::Text(current_label(state))
}

fn fill_rect(fb: &mut [u8], rect: Rect, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let stride = rect.width as usize;
    if stride == 0 {
        return;
    }
    let x0 = rect.x + x;
    let y0 = rect.y + y;
    let x1 = (x0 + w).min(rect.x + rect.width as i32);
    let y1 = (y0 + h).min(rect.y + rect.height as i32);

    for yy in y0.max(0)..y1 {
        for xx in x0.max(0)..x1 {
            let offset = (yy as usize * stride + xx as usize) * 4;
            if offset + 3 < fb.len() {
                fb[offset] = (color >> 16) as u8;
                fb[offset + 1] = (color >> 8) as u8;
                fb[offset + 2] = color as u8;
                fb[offset + 3] = (color >> 24) as u8;
            }
        }
    }
}

fn draw_rect_outline(
    fb: &mut [u8],
    rect: Rect,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    light: u32,
    shadow: u32,
) {
    fill_rect(fb, rect, x, y, w, 1, light);
    fill_rect(fb, rect, x, y, 1, h, light);
    fill_rect(fb, rect, x, y + h - 1, w, 1, shadow);
    fill_rect(fb, rect, x + w - 1, y, 1, h, shadow);
}

fn draw_text_clipped(
    fb: &mut [u8],
    rect: Rect,
    start_x: i32,
    start_y: i32,
    max_width: i32,
    text: &str,
    color: (u8, u8, u8),
) {
    let mut x = start_x;
    let stride = rect.width as usize;

    for ch in text.chars() {
        if x - start_x >= max_width {
            break;
        }
        draw_char(fb, rect, stride, x, start_y, ch, color);
        x += 8;
    }
}

fn draw_char(
    fb: &mut [u8],
    rect: Rect,
    stride: usize,
    x: i32,
    y: i32,
    ch: char,
    color: (u8, u8, u8),
) {
    if let Some(glyph) = get_glyph(ch) {
        for row in 0..16 {
            for col in 0..8 {
                if glyph.get_pixel(col, row) {
                    let px = rect.x + x + col as i32;
                    let py = rect.y + y + row as i32;
                    if px < 0
                        || py < 0
                        || px >= rect.x + rect.width as i32
                        || py >= rect.y + rect.height as i32
                    {
                        continue;
                    }
                    let offset = (py as usize * stride + px as usize) * 4;
                    if offset + 3 < fb.len() {
                        fb[offset] = color.2;
                        fb[offset + 1] = color.1;
                        fb[offset + 2] = color.0;
                        fb[offset + 3] = 0xFF;
                    }
                }
            }
        }
    }
}

fn draw_arrow(fb: &mut [u8], rect: Rect, area_x: i32, height: i32, area_width: i32) {
    let center_x = rect.x + area_x + area_width / 2;
    let center_y = rect.y + height / 2;
    let size = if area_width < 12 { 3 } else { 5 };

    for row in 0..size {
        let span = row * 2 + 1;
        let start_x = center_x - row;
        let y = center_y + row;
        for col in 0..span {
            let px = start_x + col;
            let py = y;
            let stride = rect.width as usize;
            if px < rect.x || py < rect.y || px >= rect.x + rect.width as i32 {
                continue;
            }
            let offset = (py as usize * stride + px as usize) * 4;
            if offset + 3 < fb.len() {
                fb[offset] = 0x60;
                fb[offset + 1] = 0x60;
                fb[offset + 2] = 0x60;
                fb[offset + 3] = 0xFF;
            }
        }
    }
}
