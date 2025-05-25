use alloc::{string::String, vec, vec::Vec};
use embedded_graphics::{
    geometry::{Point, Size},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};

#[derive(Debug)]
pub struct Surface {
    pub position: Point,
    pub size: Size,
    pub buffer: Vec<Rgb565>,
    pub dirty: Vec<Rectangle>,
    pub label: Option<String>,
}

impl Surface {
    pub fn new(position: Point, size: Size, label: Option<String>) -> Self {
        let buffer = vec![Rgb565::BLACK; (size.width * size.height) as usize];
        Self {
            position,
            size,
            buffer,
            dirty: Vec::new(),
            label,
        }
    }

    pub fn mark_dirty(&mut self, rect: Rectangle) {
        self.dirty.push(rect);
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }

    pub fn draw_to<T>(&self, target: &mut T)
    where
        T: DrawTarget<Color = Rgb565, Error = core::convert::Infallible>,
    {
        // For now, just blit the whole buffer at self.position
        let origin = self.position;
        for y in 0..self.size.height {
            for x in 0..self.size.width {
                let index = (y * self.size.width + x) as usize;
                let color = self.buffer[index];
                let point = origin + Point::new(x as i32, y as i32);
                let _ = target.draw_iter(core::iter::once(Pixel(point, color)));
            }
        }
    }
}
