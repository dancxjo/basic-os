use core::borrow::BorrowMut;
use core::cell::RefCell;

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use log::info;
use serde::Serialize;

use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, Moment, RTC};
use crate::gui::GUI;
use crate::interrupts::init_interrupts;
use crate::memory;
use crate::names::PersonaName;
use crate::penalty_task::PenaltyTask;
use crate::scheduler::Scheduler;
use crate::{
    framebuffer::Framebuffer,
    gdt::init_gdt,
    idt::{init_double_fault_stack, init_idt},
};

/// Target frame time: 60 FPS
const FRAME_BUDGET_NS: u64 = 16_666_667;

// This is the thingifiable Kernel
#[derive(Debug, Serialize, Clone, Copy)]
pub struct Kernel {
    tick_count: u64,
    boot_time: Moment,
}

impl Kernel {
    pub fn new(tick_count: u64, boot_time: Moment) -> Self {
        Kernel {
            tick_count,
            boot_time,
        }
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count().wrapping_add(1);
    }

    pub fn boot_time(&self) -> Moment {
        self.boot_time
    }
}

pub trait Task {
    fn tick(&mut self);
    fn done(&mut self) -> bool;
}

// This is the true kernel
pub struct Core {
    kernel: Kernel,
    space: things::Space,
    mapper: &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
    frame_allocator: memory::BootFrameAllocator,
    framebuffer: Framebuffer,
    clock: Clock,
    scheduler: Scheduler,
}

impl Core {
    fn refresh(&mut self) {
        self.framebuffer.flush();
    }

    pub fn spin(&mut self) -> ! {
        log::info!("Kernel spin loop started");

        // TODO: This is the only place to do late initialization
        self.init_late();
        self.announce_boot();

        let mut last_cycle = self.clock.nanos_since_boot();
        const FRAMERATE: u64 = 60;
        loop {
            // Framebuffer gets priority
            let current_cycle = self.clock.nanos_since_boot();
            if current_cycle - last_cycle >= 1_000_000_000 / FRAMERATE {
                self.refresh();
                last_cycle = current_cycle;
            }
            self.scheduler.tick(&mut self.clock);
            self.kernel.tick_count = self.kernel.tick_count.wrapping_add(1);
        }
    }

    fn init_late(&mut self) {
        let kernel = things::Thing::new(self.kernel);
        let kid = kernel.id.clone();
        self.space.insert(kernel);
        let once_upon_a_time = things::Thing::new(self.clock.booted_at());
        let mid = once_upon_a_time.id.clone();
        self.space.insert(once_upon_a_time);
        let boots_at = things::Thing::new(things::Verb::from_regular("(at time) boot"));
        let bid = boots_at.id.clone();
        self.space.insert(boots_at);
        let i_wuz_here = things::Fact::new(kid, bid, mid, false);
        self.space.insert(things::Thing::new(i_wuz_here));

        let gui = Box::new(GUI::new(&self.framebuffer));
        let penalty_task = PenaltyTask::new(gui, Some("gooey".to_string()));
        self.scheduler.add_task(penalty_task);
    }

    fn announce_boot(&mut self) {
        let all_facts = self.space.the::<things::Fact>().all();

        for fact in all_facts {
            let this = self.space.get(fact.subject);
            let that = self.space.get(fact.object);
            let does = self.space.get(fact.verb);

            let s = this
                .map(|t| format!("{}", short_hash(t.id)))
                .unwrap_or_else(|| "something".to_string());

            let o = that
                .map(|t| format!("{}", short_hash(t.id)))
                .unwrap_or_else(|| "something".to_string());

            let v = does
                .and_then(|d| d.data.as_any().downcast_ref::<things::Verb>())
                .map(|verb| verb.conjugated())
                .unwrap_or_else(|| "does something with".to_string());

            info!("{} {} {}", s, v, o);
        }
    }

    pub fn new() -> Self {
        log::info!("Creating new Kernel instance");
        let (mapper, frame_allocator) = Self::init_memory();
        let framebuffer = Self::init_framebuffer();
        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Clock::new(hpet, rtc);
        let kernel = Kernel::new(0, clock.current_time());

        let core = Core {
            kernel,
            mapper,
            frame_allocator,
            framebuffer,
            clock,
            space: things::Space::new(),
            scheduler: Scheduler::new(FRAME_BUDGET_NS * 100),
        };

        core
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
}

use num_bigint::BigUint;
use uuid::Uuid;

fn short_hash1(uuid: Uuid) -> String {
    let bytes = &uuid.as_bytes()[..6]; // take only the first 6 bytes = 48 bits
    let big = BigUint::from_bytes_be(bytes);
    big.to_str_radix(36)
}

fn short_hash(uuid: Uuid) -> String {
    let name = PersonaName::from_uuid(uuid);
    name.to_string()
}
