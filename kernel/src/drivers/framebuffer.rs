use limine::request::FramebufferRequest;

const MAX_WIDTH: usize = 3840;
const MAX_HEIGHT: usize = 2160;

#[derive(Debug)]
pub struct Framebuffer {
    fb: &'static mut [u32],
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub pitch_pixels: usize,
    pub bpp: u16,
}

impl Framebuffer {
    pub fn new() -> Option<Self> {
        static FB_REQUEST: FramebufferRequest = FramebufferRequest::new();
        let response = FB_REQUEST.get_response()?;
        let fb_info = response.framebuffers().next()?;

        let width = fb_info.width() as usize;
        let height = fb_info.height() as usize;
        let pitch = fb_info.pitch() as usize;
        let pitch_pixels = pitch / 4;
        let len = pitch_pixels * height;
        let fb_ptr = fb_info.addr() as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };

        Some(Self {
            fb: fb_slice,
            width,
            height,
            pitch,
            pitch_pixels,
            bpp: fb_info.bpp(),
        })
    }

    pub fn fb_len(&self) -> usize {
        self.fb.len()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pitch_pixels(&self) -> usize {
        self.pitch_pixels
    }

    pub fn blit(&mut self, x: usize, y: usize, w: usize, h: usize, data: &[u32]) -> Result<(), ()> {
        if x + w > self.width || y + h > self.height {
            return Err(());
        }

        let start = y * self.pitch_pixels + x;
        for row in 0..h {
            let dest =
                &mut self.fb[start + row * self.pitch_pixels..start + row * self.pitch_pixels + w];
            dest.copy_from_slice(&data[row * w..row * w + w]);
        }
        Ok(())
    }
}
