use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use uuid::Uuid;

use crate::bitmap::Bitmap;
use crate::scene::{Scene, SceneItem};
use crate::types::{Rect, Rgba};

#[derive(Clone, Debug)]
pub enum LayerKind {
    SolidColor(Rgba),
    Image(String),
}

#[derive(Clone, Debug)]
pub struct WallpaperState {
    pub id: Uuid,
    pub mode_node: Uuid,
    pub layers: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct LayerState {
    pub id: Uuid,
    pub kind: LayerKind,
    pub scroll_factor_x: f64,
    pub scroll_factor_y: f64,
    pub z_index: usize,
}

pub fn build_wallpaper_scene(
    scene: &mut Scene,
    screen_rect: Rect,
    wallpaper: &WallpaperState,
    layer_lookup: &BTreeMap<Uuid, LayerState>,
    bitmaps: &BTreeMap<String, Arc<Bitmap>>,
    scroll_origin: (f64, f64),
) {
    let mut layers: Vec<&LayerState> = wallpaper
        .layers
        .iter()
        .filter_map(|id| layer_lookup.get(id))
        .collect();

    layers.sort_by_key(|l| l.z_index);

    for layer in layers {
        match &layer.kind {
            LayerKind::SolidColor(color) => {
                scene.push(SceneItem::FillRect {
                    rect: screen_rect,
                    color: *color,
                });
            }
            LayerKind::Image(name) => {
                if let Some(bitmap) = bitmaps.get(name) {
                    let offset_x = (scroll_origin.0 * layer.scroll_factor_x) as i32;
                    let offset_y = (scroll_origin.1 * layer.scroll_factor_y) as i32;

                    scene.push(SceneItem::BlitImage {
                        rect: screen_rect,
                        image: bitmap.clone(),
                        repeat: true,
                        offset: (offset_x, offset_y),
                    });
                }
            }
        }
    }
}
