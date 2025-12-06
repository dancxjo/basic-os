use crate::Rect;
use alloc::vec::Vec;
use uuid::Uuid;

pub use userland::flex::{AlignItems, FlexDirection, FlexWrap, JustifyContent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutSpec {
    Stack,
    Row,
    Column,
    Grid {
        rows: usize,
        cols: usize,
    },
    Flex {
        direction: FlexDirection,
        wrap: FlexWrap,
        justify: JustifyContent,
        align: AlignItems,
    },
}

#[derive(Clone, Debug)]
pub struct LayoutItem {
    pub id: Uuid,
    pub min_width: u32,
    pub min_height: u32,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl Default for LayoutItem {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            min_width: 0,
            min_height: 0,
            max_width: None,
            max_height: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
        }
    }
}

pub fn layout(
    container: Rect,
    spec: LayoutSpec,
    children: &[LayoutItem],
    gap: i32,
) -> Vec<(Uuid, Rect)> {
    let mut result = Vec::new();
    if children.is_empty() {
        return result;
    }

    match spec {
        LayoutSpec::Stack => {
            for child in children {
                result.push((child.id, container));
            }
        }
        LayoutSpec::Row => {
            // Legacy Row (equal width)
            let count = children.len() as u32;
            let total_gap = (count.saturating_sub(1) as i32) * gap;
            let available_width = (container.width as i32 - total_gap).max(0) as u32;
            let item_width = available_width / count;

            let mut x = container.x;
            for (i, child) in children.iter().enumerate() {
                let w = if i == (count - 1) as usize {
                    container.width - (item_width * (count - 1)) - (total_gap as u32)
                } else {
                    item_width
                };
                result.push((child.id, Rect::new(x, container.y, w, container.height)));
                x += (w as i32) + gap;
            }
        }
        LayoutSpec::Column => {
            // Legacy Column (equal height)
            let count = children.len() as u32;
            let total_gap = (count.saturating_sub(1) as i32) * gap;
            let available_height = (container.height as i32 - total_gap).max(0) as u32;
            let item_height = available_height / count;

            let mut y = container.y;
            for (i, child) in children.iter().enumerate() {
                let h = if i == (count - 1) as usize {
                    container.height - (item_height * (count - 1)) - (total_gap as u32)
                } else {
                    item_height
                };
                result.push((child.id, Rect::new(container.x, y, container.width, h)));
                y += (h as i32) + gap;
            }
        }
        LayoutSpec::Grid { rows, cols } => {
            let rows = rows.max(1) as u32;
            let cols = cols.max(1) as u32;
            let total_gap_x = (cols.saturating_sub(1) as i32) * gap;
            let total_gap_y = (rows.saturating_sub(1) as i32) * gap;
            let available_width = (container.width as i32 - total_gap_x).max(0) as u32;
            let available_height = (container.height as i32 - total_gap_y).max(0) as u32;
            let item_width = available_width / cols;
            let item_height = available_height / rows;

            for (i, child) in children.iter().enumerate() {
                let row = (i as u32) / cols;
                let col = (i as u32) % cols;
                if row >= rows {
                    break;
                }
                let x = container.x + (col as i32 * (item_width as i32 + gap));
                let y = container.y + (row as i32 * (item_height as i32 + gap));
                let mut w = if col == cols - 1 {
                    container.width - (item_width * (cols - 1)) - (total_gap_x as u32)
                } else {
                    item_width
                };
                let mut h = if row == rows - 1 {
                    container.height - (item_height * (rows - 1)) - (total_gap_y as u32)
                } else {
                    item_height
                };

                if let Some(max_w) = child.max_width {
                    w = w.min(max_w);
                }
                if let Some(max_h) = child.max_height {
                    h = h.min(max_h);
                }

                result.push((child.id, Rect::new(x, y, w, h)));
            }
        }
        LayoutSpec::Flex {
            direction,
            wrap,
            justify,
            align,
        } => {
            let is_row = direction == FlexDirection::Row;
            let main_size = if is_row {
                container.width
            } else {
                container.height
            };

            // Break children into lines
            let mut lines: Vec<Vec<&LayoutItem>> = Vec::new();
            if wrap == FlexWrap::NoWrap {
                lines.push(children.iter().collect());
            } else {
                let mut current_line = Vec::new();
                let mut current_main_used = 0;

                for child in children {
                    let basis = if is_row {
                        child.min_width
                    } else {
                        child.min_height
                    };
                    let gap_needed = if current_line.is_empty() {
                        0
                    } else {
                        gap as u32
                    };

                    if !current_line.is_empty()
                        && current_main_used + gap_needed + basis > main_size
                    {
                        lines.push(current_line);
                        current_line = Vec::new();
                        current_main_used = 0;
                    }

                    if !current_line.is_empty() {
                        current_main_used += gap as u32;
                    }
                    current_main_used += basis;
                    current_line.push(child);
                }
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
            }

            let mut current_cross_pos = 0;

            for line in lines {
                let count = line.len();
                let total_gap = (count.saturating_sub(1) as i32 * gap).max(0) as u32;

                // Calculate total basis and grow factors for this line
                let mut total_basis = 0;
                let mut total_grow = 0.0;
                let mut line_cross_size = 0;

                for child in &line {
                    let basis = if is_row {
                        child.min_width
                    } else {
                        child.min_height
                    };
                    let cross = if is_row {
                        child.min_height
                    } else {
                        child.min_width
                    };

                    total_basis += basis;
                    total_grow += child.flex_grow;
                    line_cross_size = line_cross_size.max(cross);
                }

                // If NoWrap, force line cross size to container cross size
                if wrap == FlexWrap::NoWrap {
                    line_cross_size = if is_row {
                        container.height
                    } else {
                        container.width
                    };
                }

                let available_space = main_size.saturating_sub(total_basis + total_gap);

                let mut current_main_pos = 0;

                // Handle JustifyContent (if no grow)
                if total_grow == 0.0 && available_space > 0 {
                    match justify {
                        JustifyContent::Start => {}
                        JustifyContent::Center => current_main_pos = available_space / 2,
                        JustifyContent::End => current_main_pos = available_space,
                        JustifyContent::SpaceBetween => {
                            // Handled in loop by adjusting gap
                        }
                        JustifyContent::SpaceAround => {
                            current_main_pos = available_space / (count as u32 * 2);
                        }
                    }
                }

                let extra_gap = if total_grow == 0.0
                    && available_space > 0
                    && justify == JustifyContent::SpaceBetween
                    && count > 1
                {
                    available_space / (count - 1) as u32
                } else if total_grow == 0.0
                    && available_space > 0
                    && justify == JustifyContent::SpaceAround
                {
                    available_space / count as u32
                } else {
                    0
                };

                let effective_gap = gap as u32 + extra_gap;

                for child in line {
                    let basis = if is_row {
                        child.min_width
                    } else {
                        child.min_height
                    };
                    let grow_share = if total_grow > 0.0 {
                        (available_space as f32 * (child.flex_grow / total_grow)) as u32
                    } else {
                        0
                    };

                    let mut item_main_size = basis + grow_share;

                    // Cross axis alignment
                    let mut item_cross_size = if align == AlignItems::Stretch {
                        line_cross_size
                    } else {
                        if is_row {
                            child.min_height
                        } else {
                            child.min_width
                        }
                    };

                    // Apply max constraints
                    if is_row {
                        if let Some(max_w) = child.max_width {
                            item_main_size = item_main_size.min(max_w);
                        }
                        if let Some(max_h) = child.max_height {
                            item_cross_size = item_cross_size.min(max_h);
                        }
                    } else {
                        if let Some(max_h) = child.max_height {
                            item_main_size = item_main_size.min(max_h);
                        }
                        if let Some(max_w) = child.max_width {
                            item_cross_size = item_cross_size.min(max_w);
                        }
                    }

                    let cross_offset = match align {
                        AlignItems::Start | AlignItems::Stretch => 0,
                        AlignItems::Center => (line_cross_size.saturating_sub(item_cross_size)) / 2,
                        AlignItems::End => line_cross_size.saturating_sub(item_cross_size),
                    };

                    let (x, y, w, h) = if is_row {
                        (
                            container.x + current_main_pos as i32,
                            container.y + current_cross_pos as i32 + cross_offset as i32,
                            item_main_size,
                            item_cross_size,
                        )
                    } else {
                        (
                            container.x + current_cross_pos as i32 + cross_offset as i32,
                            container.y + current_main_pos as i32,
                            item_cross_size,
                            item_main_size,
                        )
                    };

                    result.push((child.id, Rect::new(x, y, w, h)));

                    current_main_pos += item_main_size + effective_gap;
                    if justify == JustifyContent::SpaceAround {
                        current_main_pos += extra_gap; // Add second half of space
                    }
                }

                current_cross_pos += line_cross_size + gap as u32;
            }
        }
    }

    result
}
