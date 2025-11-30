use alloc::sync::Arc;
use alloc::{collections::BTreeMap, string::ToString};
use core::cmp::{max, min};
use core::ops::Range;
use font8x8::{BASIC_FONTS, UnicodeFonts};
use limine::request::FramebufferRequest;
use spin::Mutex as SpinMutex;
use x86_64::VirtAddr;

use crate::drivers::device::{self, DeviceKind};
use crate::graph::{self, GraphFiatRequest, canon};
use thing_abi::Value;
use uuid::Uuid;

static FRAMEBUFFER_VIRT_RANGE: SpinMutex<Option<Range<VirtAddr>>> = SpinMutex::new(None);
static FRAMEBUFFER_REGION: SpinMutex<Option<(u64, usize)>> = SpinMutex::new(None);
static FRAMEBUFFER_DEVICE: SpinMutex<Option<Arc<SpinMutex<Framebuffer>>>> = SpinMutex::new(None);

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FramebufferInfo {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u64,
    pub addr: u64,
}

static FRAMEBUFFER_INFO: SpinMutex<Option<FramebufferInfo>> = SpinMutex::new(None);

pub fn get_framebuffer_info() -> Option<FramebufferInfo> {
    *FRAMEBUFFER_INFO.lock()
}

pub fn get_framebuffer_virt_range() -> Option<Range<VirtAddr>> {
    FRAMEBUFFER_VIRT_RANGE.lock().clone()
}

fn set_framebuffer_region(base: u64, len: usize) {
    *FRAMEBUFFER_REGION.lock() = Some((base, len));
}

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
        let virt_addr = fb_info.addr();

        let info = FramebufferInfo {
            width: width as u64,
            height: height as u64,
            pitch: pitch as u64,
            bpp: fb_info.bpp() as u64,
            addr: virt_addr as u64,
        };
        *FRAMEBUFFER_INFO.lock() = Some(info);

        *FRAMEBUFFER_VIRT_RANGE.lock() = Some(
            VirtAddr::new(virt_addr as u64)..VirtAddr::new((virt_addr as u64) + (len * 4) as u64),
        );

        let fb_ptr = virt_addr as *mut u32;
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb_ptr, len) };
        set_framebuffer_region(virt_addr as u64, len * core::mem::size_of::<u32>());

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
}

fn map_framebuffer() -> Option<(u64, usize)> {
    FRAMEBUFFER_REGION.lock().clone()
}

fn write_framebuffer(buf: &[u8]) -> usize {
    let fb_arc = match FRAMEBUFFER_DEVICE.lock().clone() {
        Some(fb) => fb,
        None => return 0,
    };
    let mut fb = fb_arc.lock();
    let dst = unsafe {
        core::slice::from_raw_parts_mut(
            fb.fb.as_mut_ptr() as *mut u8,
            fb.fb_len() * core::mem::size_of::<u32>(),
        )
    };
    let count = min(buf.len(), dst.len());
    if count > 0 {
        dst[..count].copy_from_slice(&buf[..count]);
    }
    count
}

fn read_framebuffer(buf: &mut [u8]) -> usize {
    let fb_arc = match FRAMEBUFFER_DEVICE.lock().clone() {
        Some(fb) => fb,
        None => return 0,
    };
    let fb = fb_arc.lock();
    let required = core::mem::size_of::<u32>() * 4;
    if buf.len() < required {
        return 0;
    }

    let width = (fb.width as u32).to_le_bytes();
    let height = (fb.height as u32).to_le_bytes();
    let pitch = (fb.pitch as u32).to_le_bytes();
    let bpp = (fb.bpp as u32).to_le_bytes();

    buf[..4].copy_from_slice(&width);
    buf[4..8].copy_from_slice(&height);
    buf[8..12].copy_from_slice(&pitch);
    buf[12..16].copy_from_slice(&bpp);
    required
}

/// Advertise the framebuffer as a device endpoint so userland drivers can map
/// or push pixel data directly.
pub fn register_framebuffer_device(framebuffer: Arc<SpinMutex<Framebuffer>>) {
    let fb = framebuffer.lock();
    let mut fields = BTreeMap::new();
    fields.insert(canon::WIDTH, Value::U64(fb.width as u64));
    fields.insert(canon::HEIGHT, Value::U64(fb.height as u64));
    fields.insert(canon::PITCH, Value::U64(fb.pitch as u64));
    fields.insert(canon::BPP, Value::U64(fb.bpp as u64));
    fields.insert(canon::KIND, Value::Text("framebuffer".into()));
    fields.insert(canon::MODE, Value::Text("native".into()));
    if let Some((addr, _len)) = *FRAMEBUFFER_REGION.lock() {
        fields.insert(canon::ADDR, Value::U64(addr));
    }
    let node = device::create_device_node(canon::DEVICE, device::FRAMEBUFFER_DEVICE_NAME, fields);
    drop(fb);
    *FRAMEBUFFER_DEVICE.lock() = Some(framebuffer);
    device::register_device(
        DeviceKind::Framebuffer,
        Some(read_framebuffer),
        Some(write_framebuffer),
        Some(map_framebuffer),
        Some(node),
    );
}
