use crate::allocator::{BootFrameAllocator, init_heap, init_paging};
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gdt::init_gdt;
use crate::graph::{Graph, bootstrap_graph};
use crate::idt::init_idt;
use crate::input::{KEYBOARD_BUFFER, KEYBOARD_HEAD};
use crate::interrupts::init_interrupts;
use crate::mouse::Mouse;
use crate::scheduler::{TaskState, WasmTask};
use crate::stack::init_kernel_stack;
use crate::{bootstrap_step, ps2, serial_print};
use alloc::sync::Arc;
use log::info;
use spin::Mutex as SpinMutex;
use spin::Mutex;
use wasmi::{Config, Engine, Linker, Module, Store};
use x86_64::instructions::interrupts;

pub(crate) static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

pub struct System {
    mouse: Arc<SpinMutex<Mouse>>,
    framebuffer: Arc<SpinMutex<Framebuffer>>,
    clock: Arc<SpinMutex<Clock>>,
    graph: Arc<SpinMutex<Graph>>,
    keyboard_index: usize,
    mouse_index: usize,
    scheduler: Arc<SpinMutex<crate::scheduler::Scheduler>>,
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

        let framebuffer = bootstrap_step!("framebuffer", {
            Arc::new(SpinMutex::new(
                Framebuffer::new().expect("Framebuffer not available"),
            ))
        });

        let mouse = bootstrap_step!("PS/2 devices", {
            ps2::enable_ps2_devices();
            Arc::new(SpinMutex::new(Mouse::new(&framebuffer.lock())))
        });

        let scheduler = bootstrap_step!("scheduler", {
            Arc::new(SpinMutex::new(crate::scheduler::Scheduler::new()))
        });

        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Arc::new(SpinMutex::new(Clock::new(hpet, rtc)));
        info!("ThingOS initialized.");
        Self {
            mouse,
            framebuffer,
            clock,
            graph,
            keyboard_index: 0,
            mouse_index: 0,
            scheduler,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        interrupts::enable();

        let mut scheduler = self.scheduler.lock();
        let compositor_task = self
            .create_compositor_task(0)
            .expect("Failed to create compositor task");
        scheduler.add_task(compositor_task);
        for i in 0..4 {
            let task = self
                .create_hello_task(i + 1)
                .expect("Failed to create hello task");
            scheduler.add_task(task);
        }
        loop {
            scheduler.schedule();
            info!("tick");
        }
    }

    pub fn create_compositor_task(&self, pid: usize) -> anyhow::Result<WasmTask> {
        let wasm_bytes = crate::bootloader::get_module("/boot/compositor.wasm")
            .ok_or_else(|| anyhow::anyhow!("Failed to load compositor.wasm"))?;

        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let mut store = Store::new(&engine, ());
        let module = Module::new(&engine, &wasm_bytes)?;
        let mut linker = Linker::new(&engine);
        linker.func_wrap("env", "putchar", |_: wasmi::Caller<'_, ()>, ch: i32| {
            serial_print!("{}", ch as u8 as char);
        })?;
        linker.func_wrap(
            "env",
            "blit",
            |caller: wasmi::Caller<'_, ()>,
             x: u32,
             y: u32,
             w: u32,
             h: u32,
             buf_ptr: u32|
             -> Result<i32, wasmi::core::Trap> {
                let len = (w * h * 4) as usize;

                // Get memory export
                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| wasmi::core::Trap::new("failed to find memory"))?;

                // Read from guest memory
                let mut raw = vec![0u8; len];
                memory.read(&caller, buf_ptr as usize, &mut raw)?;

                // Interpret as &[u32]
                let buf_u32: &[u32] = bytemuck::cast_slice(&raw);

                // Blit to framebuffer
                let system_guard = SYSTEM.lock();
                let mut framebuffer = system_guard.as_ref().unwrap().framebuffer.lock();
                if let Err(_) =
                    framebuffer.blit(x as usize, y as usize, w as usize, h as usize, buf_u32)
                {
                    log::warn!("blit failed at ({},{}) with size {}x{}", x, y, w, h);
                    return Ok(-1);
                }
                Ok(0)
            },
        )?;

        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

        Ok(WasmTask {
            pid,
            store,
            instance,
            state: TaskState::Runnable,
        })
    }

    pub fn create_hello_task(&self, pid: usize) -> anyhow::Result<WasmTask> {
        let wasm_bytes = crate::bootloader::get_module("/boot/hello_from.wasm")
            .ok_or_else(|| anyhow::anyhow!("Failed to load hello_from.wasm"))?;

        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let mut store = Store::new(&engine, ());
        let module = Module::new(&engine, &wasm_bytes)?;
        let mut linker = Linker::new(&engine);

        linker.func_wrap("env", "putchar", |_: wasmi::Caller<'_, ()>, ch: i32| {
            serial_print!("{}", ch as u8 as char);
        })?;

        linker.func_wrap(
            "env",
            "get_pid",
            move |_caller: wasmi::Caller<'_, ()>| -> i32 { pid as i32 },
        )?;

        linker.func_wrap("env", "get_char", |_caller: wasmi::Caller<'_, ()>| -> i32 {
            let head = KEYBOARD_HEAD.load(core::sync::atomic::Ordering::Acquire);
            let buf = KEYBOARD_BUFFER.lock();
            buf[head % 256] as i32
        })?;

        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

        Ok(WasmTask {
            pid,
            store,
            instance,
            state: TaskState::Runnable,
        })
    }
}

pub fn init_and_run_system() -> ! {
    let system = System::boot();
    *SYSTEM.lock() = Some(system);
    SYSTEM.lock().as_mut().unwrap().run()
}
