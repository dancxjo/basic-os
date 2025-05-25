use core::convert::Infallible;

use alloc::vec::Vec;
use embedded_graphics::{pixelcolor::Rgb565, prelude::DrawTarget};

use crate::compositor::surface::Surface;

pub struct Layer {
    pub z_index: isize,
    pub surface: Surface,
}

pub struct Compositor {
    pub layers: Vec<Layer>,
}

impl Compositor {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    pub fn add_layer(&mut self, z_index: isize, surface: Surface) {
        self.layers.push(Layer { z_index, surface });
        self.layers.sort_by_key(|l| l.z_index);
    }

    pub fn draw_to<T>(&mut self, target: &mut T)
    where
        T: DrawTarget<Color = Rgb565, Error = Infallible>,
    {
        for layer in self.layers.iter() {
            layer.surface.draw_to(target);
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}
