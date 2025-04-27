use core::cell::RefCell;

use crate::beat::Beat;
use crate::bootloader::get_hhdm_offset;
use crate::clock::{Clock, HPET, RTC};
use crate::framebuffer::Framebuffer;
use crate::gui::GUI;
use crate::mouse::Mouse;
use crate::pattern::Pattern;
use crate::screen::Screen;
use crate::space::Space;
use crate::thing::{Fact, Thing};
use crate::verb::Verb;

use alloc::rc::Rc;
use alloc::vec::Vec;
use alloc::{format, vec};
use log::{debug, info};
use uuid::Uuid;

/// Target frame time: 60 FPS
const FRAME_BUDGET_NS: u64 = 16_666_667;

pub struct ThingOS {
    space: Rc<RefCell<Space>>,
    clock: Clock,
    framebuffer: Rc<RefCell<Framebuffer>>,
    screen_id: Uuid,
    mouse_id: Uuid,
    gui: Rc<RefCell<GUI>>,
}

impl ThingOS {
    pub fn new() -> Self {
        info!("Bootstrapping ThingOS...");

        let (mapper, mut frame_allocator) = unsafe { crate::memory::init(get_hhdm_offset()) };
        crate::gdt::init_gdt();
        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        crate::idt::init_double_fault_stack(tss, mapper, &mut frame_allocator);
        crate::idt::init_idt();
        crate::interrupts::init_interrupts();
        x86_64::instructions::interrupts::enable();

        let framebuffer = Rc::new(RefCell::new(
            Framebuffer::new().expect("Framebuffer not available"),
        ));
        let hpet = HPET::new(0xFED00000);
        let rtc = RTC::new();
        let clock = Clock::new(hpet, rtc);

        let space = Rc::new(RefCell::new(Space::new()));
        let mouse = Mouse::new(&framebuffer.borrow());
        let mouse_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"mouse");
        let mouse_thing = Thing::new(mouse);
        space.borrow_mut().insert(mouse_thing);
        // Screen setup
        let screen = Screen::new(&framebuffer.borrow());
        let screen_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"screen");

        // GUI setup
        let gui = Rc::new(RefCell::new(GUI::new(&framebuffer.borrow())));
        let verb = Verb::from_regular("is_ready_to_draw_to");
        let verb_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, verb.name.as_bytes());
        let gui_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"gui");
        gui.borrow_mut().set_ids(gui_id, verb_id, screen_id);

        info!("ThingOS initialized.");

        Self {
            space,
            clock,
            framebuffer,
            screen_id,
            gui,
            mouse_id,
        }
    }

    pub fn run(&mut self) -> ! {
        info!("ThingOS running...");
        let mut last_cycle = self.clock.nanos_since_boot();
        let space = self.space.borrow();
        let mut space_mut = self.space.borrow_mut();

        loop {
            let now = self.clock.nanos_since_boot();
            let beat_facts = beat_all_things(&mut space);
            space.commit(beat_facts);
        }
    }
}
