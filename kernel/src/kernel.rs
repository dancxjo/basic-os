use crate::bootloader::get_hhdm_offset;
use crate::framebuffer::draw_kernel_ui;
use crate::hpet::HPET;
use crate::interrupts::init_interrupts;
use crate::memory;
use crate::{
    framebuffer::Framebuffer,
    gdt::init_gdt,
    idt::{init_double_fault_stack, init_idt},
};

pub struct Kernel {
    tick_count: u64,
    mapper: &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
    frame_allocator: memory::BootFrameAllocator,
    framebuffer: Framebuffer,
    hpet: HPET,
}

impl Kernel {
    fn tick(&mut self) {
        draw_kernel_ui(&mut self.framebuffer, self.tick_count);
        self.framebuffer.flush();
    }

    pub fn spin(&mut self) -> ! {
        log::info!("Kernel spin loop started");

        loop {
            self.tick();
            self.tick_count = self.tick_count.wrapping_add(1);
        }
    }

    pub fn new() -> Self {
        log::info!("Creating new Kernel instance");
        let (mapper, frame_allocator) = Self::init_memory();
        let framebuffer = Self::init_framebuffer();
        let hpet = HPET::new();

        let kernel = Kernel {
            mapper,
            frame_allocator,
            framebuffer,
            hpet,
            tick_count: 0,
        };

        kernel
    }

    pub fn sync_clock(&self) {
        let mark1 = self.hpet.read();
        log::info!("Mark1: {}", mark1);
        // let rtc_time = 1111;
        let mark2 = self.hpet.read();
        log::info!("Mark2: {}", mark2);
        log::info!("HPET initialized");
    }

    fn init_memory() -> (
        &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
        memory::BootFrameAllocator,
    ) {
        log::info!("Initializing memory");
        let offset = get_hhdm_offset();
        let (mut mapper, mut frame_allocator) = unsafe { memory::init(offset) };
        log::info!("Paging initialized");

        init_gdt();
        log::info!("GDT initialized");

        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        init_double_fault_stack(tss, &mut mapper, &mut frame_allocator);
        log::info!("Double fault stack initialized");

        init_idt();
        init_interrupts();
        x86_64::instructions::interrupts::enable();
        log::info!("IDT and interrupts initialized");

        (mapper, frame_allocator)
    }

    fn init_framebuffer() -> Framebuffer {
        log::info!("Initializing framebuffer");
        Framebuffer::new().expect("Framebuffer not initialized")
    }

    fn print(&self) {}
}
