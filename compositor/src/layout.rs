use crate::Rect;
use alloc::vec::Vec;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutSpec {
    Stack,
    Row,
    Column,
    Grid { rows: usize, cols: usize },
}

#[derive(Clone, Debug)]
pub struct LayoutItem {
    pub id: Uuid,
    pub min_width: u32,
    pub min_height: u32,
    // We can add more constraints later (max_width, flex, etc.)
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
            let count = children.len() as u32;
            let total_gap = (count.saturating_sub(1) as i32) * gap;
            let available_width = (container.width as i32 - total_gap).max(0) as u32;
            let item_width = available_width / count;

            let mut x = container.x;
            for (i, child) in children.iter().enumerate() {
                let w = if i == (count - 1) as usize {
                    // Give remaining pixels to last item to avoid rounding gaps
                    container.width - (item_width * (count - 1)) - (total_gap as u32)
                } else {
                    item_width
                };

                result.push((child.id, Rect::new(x, container.y, w, container.height)));
                x += (w as i32) + gap;
            }
        }
        LayoutSpec::Column => {
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
                    break; // Exceeded grid capacity
                }

                let x = container.x + (col as i32 * (item_width as i32 + gap));
                let y = container.y + (row as i32 * (item_height as i32 + gap));

                let w = if col == cols - 1 {
                    container.width - (item_width * (cols - 1)) - (total_gap_x as u32)
                } else {
                    item_width
                };

                let h = if row == rows - 1 {
                    container.height - (item_height * (rows - 1)) - (total_gap_y as u32)
                } else {
                    item_height
                };

                result.push((child.id, Rect::new(x, y, w, h)));
            }
        }
    }

    result
}
