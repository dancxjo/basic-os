// kthread.rs — Kernel Thread Management for ThingOS

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;

unsafe extern "C" {
    fn switch_context(old_sp: *mut usize, new_sp: *const usize);
}

static NEXT_THREAD_ID: AtomicUsize = AtomicUsize::new(1);

pub struct KThread {
    pub id: usize,
    pub stack: Box<[u8]>,
    pub stack_ptr: *mut usize,
}

unsafe impl Send for KThread {}

lazy_static! {
    pub static ref THREADS: Mutex<Vec<KThread>> = Mutex::new(Vec::new());
    pub static ref CURRENT_THREAD: Mutex<usize> = Mutex::new(0);
}

const STACK_SIZE: usize = 4096 * 4; // 16 KiB

pub fn spawn(entry: extern "C" fn()) -> usize {
    let mut stack = vec![0u8; STACK_SIZE].into_boxed_slice();
    let stack_top = unsafe { stack.as_mut_ptr().add(STACK_SIZE) as *mut usize };

    let fake_sp = unsafe { stack_top.offset(-1) };
    unsafe {
        *fake_sp = entry as usize;
    }

    let id = NEXT_THREAD_ID.fetch_add(1, Ordering::SeqCst);
    let thread = KThread {
        id,
        stack,
        stack_ptr: fake_sp,
    };

    THREADS.lock().push(thread);
    id
}

pub fn schedule() {
    let mut threads = THREADS.lock();
    if threads.len() <= 1 {
        return;
    }

    let mut current = CURRENT_THREAD.lock();
    let old_id = *current;
    let new_id = (old_id + 1) % threads.len();

    let old_stack_ptr = threads[old_id].stack_ptr;
    let new_stack_ptr = threads[new_id].stack_ptr;

    *current = new_id;

    if old_id != new_id {
        unsafe {
            switch_context(old_stack_ptr, new_stack_ptr);
        }
    }
}
