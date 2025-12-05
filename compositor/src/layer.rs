use alloc::string::String;
use alloc::vec::Vec;
use uuid::Uuid;

use crate::types::Rgba;

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
