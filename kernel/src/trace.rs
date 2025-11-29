use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TraceEvent {
    pub timestamp: u64,
    pub kind: u16,
    pub task_id: u16,
    pub arg: u64,
}

impl Default for TraceEvent {
    fn default() -> Self {
        Self {
            timestamp: 0,
            kind: 0,
            task_id: 0,
            arg: 0,
        }
    }
}

pub const TRACE_LEN: usize = 256;

pub struct TraceBuffer {
    pub head: AtomicUsize,
    pub events: [TraceEvent; TRACE_LEN],
}

pub static mut TRACE_BUFFER: TraceBuffer = TraceBuffer {
    head: AtomicUsize::new(0),
    events: [TraceEvent {
        timestamp: 0,
        kind: 0,
        task_id: 0,
        arg: 0,
    }; TRACE_LEN],
};

#[repr(u16)]
pub enum TraceKind {
    SwitchTo = 1,
    SyscallEnter = 2,
    PageFault = 3,
    DoubleFault = 4,
    Timer = 5,
}

pub fn trace_event(kind: TraceKind, task_id: u16, arg: u64) {
    let timestamp = unsafe { core::arch::x86_64::_rdtsc() };
    unsafe {
        let head_ptr = core::ptr::addr_of_mut!(TRACE_BUFFER.head);
        let idx = (*head_ptr).load(Ordering::Relaxed);

        let events_ptr = core::ptr::addr_of_mut!(TRACE_BUFFER.events);
        (*events_ptr)[idx] = TraceEvent {
            timestamp,
            kind: kind as u16,
            task_id,
            arg,
        };
        (*head_ptr).store((idx + 1) % TRACE_LEN, Ordering::Relaxed);
    }
}

pub fn dump_trace() {
    unsafe {
        crate::klog_raw!("--- TRACE DUMP ---\r\n");
        let head_ptr = core::ptr::addr_of_mut!(TRACE_BUFFER.head);
        let head = (*head_ptr).load(Ordering::Relaxed);

        let events_ptr = core::ptr::addr_of_mut!(TRACE_BUFFER.events);

        for i in 0..TRACE_LEN {
            let idx = (head + i) % TRACE_LEN;
            let evt = &(*events_ptr)[idx];
            if evt.timestamp == 0 {
                continue;
            }

            crate::drivers::serial::raw_write_hex(evt.timestamp);
            crate::klog_raw!(": ");
            crate::drivers::serial::raw_write_hex(evt.kind as u64);
            crate::klog_raw!(" [");
            crate::drivers::serial::raw_write_hex(evt.task_id as u64);
            crate::klog_raw!("] arg=");
            crate::drivers::serial::raw_write_hex(evt.arg);
            crate::klog_raw!("\r\n");
        }
        crate::klog_raw!("--- END TRACE ---\r\n");
    }
}
