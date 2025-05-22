use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, bootstrap_graph};
use crate::gui::GUI;
use crate::idt::init_idt;
use crate::input::{
    KEYBOARD_BUFFER, KEYBOARD_HEAD, MOUSE_HEAD, MOUSE_PACKET_BUFFER, MOUSE_TAIL, process_scancode,
};
use crate::interrupts::init_interrupts;
use crate::mouse::{self, Mouse};
use crate::screen::Screen;
use crate::stack::init_kernel_stack;
use crate::tasks::SCHEDULER;
use crate::{bootstrap_step, ps2, serial_println};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU8, Ordering};
use embedded_graphics::framebuffer;
use log::info;
use spin::Mutex as SpinMutex;
use x86_64::instructions::{hlt, interrupts};

use spin::Mutex;

static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    mouse: Arc<SpinMutex<Mouse>>,
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: Arc<SpinMutex<Clock>>,
    screen: Arc<SpinMutex<Screen>>,
    gui: Arc<SpinMutex<GUI>>,
    graph: Arc<SpinMutex<Graph>>,
    keyboard_index: usize,
    mouse_index: usize,
}

impl System {
    pub fn boot() -> Self {
        info!("Ready? Set? Go!");

        bootstrap_step!("GDT", {
            init_gdt();
        });

        let mut mapper = bootstrap_step!("paging", {
            let physical_memory_offset = get_hhdm_offset();
            unsafe { init_paging(physical_memory_offset) }
        });

        let mut frame_allocator = BootFrameAllocator::init();

        bootstrap_step!("stack", {
            unsafe { init_kernel_stack(&mut mapper, &mut frame_allocator) };
        });

        bootstrap_step!("heap", {
            init_heap(&mut mapper, &mut frame_allocator);
        });

        bootstrap_step!("IDT", {
            init_idt();
        });

        bootstrap_step!("interrupts", {
            init_interrupts();
        });

        let graph = bootstrap_step!("graph", { Arc::new(SpinMutex::new(bootstrap_graph())) });

        let framebuffer = Arc::new(SpinMutex::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));

        let mouse = bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
            Arc::new(SpinMutex::new(Mouse::new(&framebuffer.lock())))
        });

        bootstrap_step!("tasks", {
            let mut scheduler = SCHEDULER.lock();
            scheduler.spawn(keyboard_thread, 0, &mut mapper, &mut frame_allocator);
            scheduler.spawn(mouse_thread, 1, &mut mapper, &mut frame_allocator);
        });

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Arc::new(SpinMutex::new(Clock::new(hpet, rtc)));
        let screen = Arc::new(SpinMutex::new(Screen::new(&framebuffer.lock())));
        let gui = Arc::new(SpinMutex::new(GUI::new(&framebuffer.lock())));
        info!("ThingOS initialized.");
        Self {
            mouse,
            framebuffer,
            clock,
            screen,
            gui,
            graph,
            keyboard_index: 0,
            mouse_index: 0,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();
        info!("Drawing GUI...");
        self.gui.lock().draw();
        {
            info!("Transferring GUI output...");
            let mut framebuffer = self.framebuffer.lock();
            info!("Framebuffer locked.");
            self.gui.lock().output(&mut framebuffer);
            info!("GUI output drawn.");
            framebuffer.flush();
            info!("Framebuffer flushed.");
            info!("GUI output transferred.");
        }

        loop {
            self.scankeys();

            if let Some((dx, dy)) = self.mouse_tick() {
                let mut mouse = self.mouse.lock();

                // Draw at the current position before updating it
                if mouse.needs_update() {
                    let mut framebuffer = self.framebuffer.lock();
                    mouse.draw(&mut framebuffer);
                }

                mouse.move_by(dx, dy);
            }

            hlt();
        }
    }

    fn scankeys(&mut self) {
        let head = KEYBOARD_HEAD.load(Ordering::Acquire);
        if head != self.keyboard_index {
            let buf = KEYBOARD_BUFFER.lock();
            for i in self.keyboard_index..head {
                let index = i % 256;
                let byte = buf[index];
                process_scancode(byte);
            }
            self.keyboard_index = head;
        }
    }
    fn mouse_tick(&mut self) -> Option<(isize, isize)> {
        let mut tail = MOUSE_TAIL.load(Ordering::Acquire);
        let head = MOUSE_HEAD.load(Ordering::Acquire);
        let mut movement = None;

        if tail == head {
            return None;
        }

        let buf = MOUSE_PACKET_BUFFER.lock();
        while tail != head {
            let index = tail % 256;
            let byte = buf[index];
            if let Some((x, y)) = self.process_mouse_packet(byte) {
                movement = Some((x, y));
            }
            tail = (tail + 1) % 256;
        }

        MOUSE_TAIL.store(tail, Ordering::Release);
        movement
    }

    fn process_mouse_packet(&self, byte: u8) -> Option<(isize, isize)> {
        static MOUSE_PACKET: [AtomicU8; 3] = [AtomicU8::new(0), AtomicU8::new(0), AtomicU8::new(0)];
        static MOUSE_PACKET_INDEX: AtomicU8 = AtomicU8::new(0);

        let idx = MOUSE_PACKET_INDEX.load(Ordering::Acquire) as usize;

        if idx == 0 && (byte & 0x08) == 0 {
            return None;
        }

        MOUSE_PACKET[idx].store(byte, Ordering::Release);
        let next_idx = idx + 1;

        if next_idx >= 3 {
            let b1 = MOUSE_PACKET[1].load(Ordering::Acquire);
            let b2 = MOUSE_PACKET[2].load(Ordering::Acquire);
            MOUSE_PACKET_INDEX.store(0, Ordering::Release);
            let dx = b1 as i8 as isize;
            let dy = -(b2 as i8 as isize);
            if dx != 0 || dy != 0 {
                return Some((dx, dy));
            }
            return None;
        } else {
            MOUSE_PACKET_INDEX.store(next_idx as u8, Ordering::Release);
        }

        None
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn keyboard_thread() {
    loop {
        if let Some(system) = SYSTEM.lock().as_mut() {
            system.scankeys();
        }
        hlt();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mouse_thread() {
    loop {
        if let Some(system) = SYSTEM.lock().as_mut() {
            system.mouse_tick();
        }
        hlt();
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}
