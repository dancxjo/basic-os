//! Minimal device registry exposed to userland through syscalls.
//! Each device is represented by a simple endpoint with optional read/write/map
//! callbacks so higher-level policy can live in userland drivers.

use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex as SpinMutex;

pub type DeviceHandle = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DeviceKind {
    Keyboard = 1,
    Mouse = 2,
    Framebuffer = 3,
    Serial = 4,
}

type ReadFn = fn(&mut [u8]) -> usize;
type WriteFn = fn(&[u8]) -> usize;
type MapFn = fn() -> Option<(u64, usize)>;

struct DeviceEndpoint {
    handle: DeviceHandle,
    kind: DeviceKind,
    read: Option<ReadFn>,
    write: Option<WriteFn>,
    map: Option<MapFn>,
}

struct DeviceTable {
    next_handle: DeviceHandle,
    endpoints: Vec<DeviceEndpoint>,
}

impl DeviceTable {
    pub fn new() -> Self {
        Self {
            next_handle: 1,
            endpoints: Vec::new(),
        }
    }

    fn alloc_handle(&mut self) -> DeviceHandle {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        handle
    }

    fn register(
        &mut self,
        kind: DeviceKind,
        read: Option<ReadFn>,
        write: Option<WriteFn>,
        map: Option<MapFn>,
    ) -> DeviceHandle {
        let handle = self.alloc_handle();
        self.endpoints.push(DeviceEndpoint {
            handle,
            kind,
            read,
            write,
            map,
        });
        handle
    }

    fn by_handle(&self, handle: DeviceHandle) -> Option<&DeviceEndpoint> {
        self.endpoints.iter().find(|e| e.handle == handle)
    }

    fn open(&self, kind: DeviceKind, index: usize) -> Option<DeviceHandle> {
        self.endpoints
            .iter()
            .filter(|e| e.kind == kind)
            .nth(index)
            .map(|e| e.handle)
    }
}

lazy_static! {
    static ref DEVICES: SpinMutex<DeviceTable> = SpinMutex::new(DeviceTable::new());
}

/// Register a new device endpoint and return its handle.
/// Safety: callbacks must be safe to invoke from syscall context.
pub fn register_device(
    kind: DeviceKind,
    read: Option<ReadFn>,
    write: Option<WriteFn>,
    map: Option<MapFn>,
) -> DeviceHandle {
    DEVICES.lock().register(kind, read, write, map)
}

/// Open a device of the requested kind at the given index.
pub fn dev_open(kind_raw: u32, index: usize) -> Option<DeviceHandle> {
    let kind = match kind_raw {
        x if x == DeviceKind::Keyboard as u32 => DeviceKind::Keyboard,
        x if x == DeviceKind::Mouse as u32 => DeviceKind::Mouse,
        x if x == DeviceKind::Framebuffer as u32 => DeviceKind::Framebuffer,
        x if x == DeviceKind::Serial as u32 => DeviceKind::Serial,
        _ => return None,
    };
    DEVICES.lock().open(kind, index)
}

/// Non-blocking read from a device; returns bytes read.
pub fn dev_read(handle: DeviceHandle, buf: &mut [u8]) -> usize {
    let devices = DEVICES.lock();
    let Some(endpoint) = devices.by_handle(handle) else {
        return 0;
    };
    let Some(read) = endpoint.read else {
        return 0;
    };
    read(buf)
}

/// Non-blocking write to a device; returns bytes written.
pub fn dev_write(handle: DeviceHandle, buf: &[u8]) -> usize {
    let devices = DEVICES.lock();
    let Some(endpoint) = devices.by_handle(handle) else {
        return 0;
    };
    let Some(write) = endpoint.write else {
        return 0;
    };
    write(buf)
}

/// Return a mapped address and length for the device, if supported.
pub fn dev_map(handle: DeviceHandle) -> Option<(u64, usize)> {
    let devices = DEVICES.lock();
    let endpoint = devices.by_handle(handle)?;
    let map = endpoint.map?;
    map()
}
