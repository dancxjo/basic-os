use core::arch::asm;

use crate::{
    bootloader, dump_overlay,
    gdt::init_gdt,
    idt::{init_double_fault_stack, init_idt},
    memory::{self, HEAP_SIZE, HEAP_START, map_page_to},
    message::Message,
    seed::SeedBlob,
    serial_println,
    thing::Graph,
};
use alloc::boxed::Box;
use limine::request::HhdmRequest;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags as Flags},
};

pub const USER_BASE_VADDR: u64 = 0x4000_0000;

pub struct Kernel {
    pub graph: Graph,
    mapper: &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
    frame_allocator: memory::BootFrameAllocator,
}

#[used]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    VirtAddr::new(resp.offset())
}

impl Kernel {
    fn get_elf_entry_point(&self) -> u64 {
        let this = self
            .graph
            .find_typed::<SeedBlob>(|_seed| true)
            .expect("Missing seed_blob");
        let data = &this.bytes;
        if &data[0..4] != b"\x7FELF" {
            panic!("Invalid ELF magic");
        }
        u64::from_le_bytes(data[24..32].try_into().unwrap()) + USER_BASE_VADDR
    }

    pub fn new() -> Self {
        let offset = get_hhdm_offset();
        let (mut mapper, mut frame_allocator) = unsafe { memory::init(offset) };
        serial_println!("Paging initialized");
        init_gdt();
        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        init_double_fault_stack(tss, &mut mapper, &mut frame_allocator);
        init_idt();
        x86_64::instructions::interrupts::enable();

        let boxed = Box::new(42_u64);
        let addr = boxed.as_ref() as *const u64 as usize;
        serial_println!("Boxed value address: {:#x}", addr);
        assert!(
            addr >= HEAP_START as usize && addr < (HEAP_START + HEAP_SIZE as u64) as usize,
            "Boxed value not in heap!"
        );

        let mut graph = Graph::new();
        dump_overlay!(&graph);
        graph.insert(
            "message",
            Message::new("ThingOS. People, places, things and ideas."),
        );
        dump_overlay!(&graph);

        Kernel {
            graph,
            mapper,
            frame_allocator,
        }
    }

    pub fn spin(&mut self) -> ! {
        dump_overlay!(&self.graph);
        self.load_hello_bin();
        loop {
            crate::panic::halt();
        }
    }

    fn load_hello_bin(&mut self) {
        if let Some(bin) = bootloader::get_module("hello-user") {
            serial_println!("Found 'hello-user' module, size {} bytes", bin.len());
            let entry = self.parse_elf(bin);
            self.map_hello_user_memory();
            self.graph.insert(
                "seed_blob",
                SeedBlob {
                    bytes: bin.to_vec(),
                },
            );
            dump_overlay!(&self.graph);
            self.sprout(entry);
        } else {
            serial_println!("Could not find 'hello-user' module");
        }
    }

    fn parse_elf(&mut self, data: &[u8]) -> u64 {
        serial_println!("[hello-user] Parsing ELF headers...");
        let magic = &data[0..4];
        if magic != b"\x7FELF" {
            serial_println!("Not a valid ELF");
            return 0;
        }

        let e_phoff = u64::from_le_bytes(data[32..40].try_into().unwrap());
        let e_phentsz = u16::from_le_bytes(data[54..56].try_into().unwrap()) as usize;
        let e_phnum = u16::from_le_bytes(data[56..58].try_into().unwrap());

        serial_println!(
            "[hello-user] PH off {:#x}, count {}, size {}",
            e_phoff,
            e_phnum,
            e_phentsz
        );

        let e_entry = u64::from_le_bytes(data[24..32].try_into().unwrap());
        let entry_point = e_entry + USER_BASE_VADDR;
        serial_println!("[hello-user] ELF entry point: {:#x}", entry_point);

        for i in 0..e_phnum {
            let off = e_phoff as usize + i as usize * e_phentsz;
            let ph = &data[off..off + e_phentsz];
            let p_type = u32::from_le_bytes(ph[0..4].try_into().unwrap());
            let p_offset = u64::from_le_bytes(ph[8..16].try_into().unwrap());
            let p_vaddr = u64::from_le_bytes(ph[16..24].try_into().unwrap()) + USER_BASE_VADDR;
            let p_filesz = u64::from_le_bytes(ph[32..40].try_into().unwrap());
            let p_memsz = u64::from_le_bytes(ph[40..48].try_into().unwrap());

            if p_type == 1 && p_memsz > 0 {
                serial_println!(
                    "[hello-user] Seg {} -> {:#x} filesz {} memsz {}",
                    i,
                    p_vaddr,
                    p_filesz,
                    p_memsz
                );
                use x86_64::structures::paging::Page;
                let start = VirtAddr::new(p_vaddr);
                let end = VirtAddr::new(p_vaddr + p_memsz - 1);
                for page in Page::range_inclusive(
                    Page::containing_address(start),
                    Page::containing_address(end),
                ) {
                    let frame = self.frame_allocator.allocate_frame().expect("no frame");
                    map_page_to(
                        self.mapper,
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                        &mut self.frame_allocator,
                    );
                }
                unsafe {
                    let dest =
                        core::slice::from_raw_parts_mut(p_vaddr as *mut u8, p_filesz as usize);
                    dest.copy_from_slice(
                        &data[p_offset as usize..(p_offset as usize + p_filesz as usize)],
                    );
                }
                // entry_point = p_vaddr; // Update entry point to the last loaded segment
            }
        }

        entry_point
    }

    fn map_hello_user_memory(&mut self) {
        serial_println!("[hello-user] Mapping user memory...");
        use x86_64::structures::paging::Page;
        let stack_start = VirtAddr::new(0x7FFF_FFFF_E000);
        let stack_end = VirtAddr::new(stack_start.as_u64() + 0x4000 - 1);
        for page in Page::range_inclusive(
            Page::containing_address(stack_start),
            Page::containing_address(stack_end),
        ) {
            let frame = self.frame_allocator.allocate_frame().expect("no frame");
            map_page_to(
                self.mapper,
                page,
                frame,
                Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
                &mut self.frame_allocator,
            );
        }
        serial_println!("[hello-user] Stack at {:#x}", stack_start);
    }

    fn sprout(&self, entry: u64) {
        let stack = 0x7FFF_FFFF_F000_u64;

        serial_println!("[sprout] Preparing to enter user mode...");
        serial_println!("[sprout] Entry point: {:#x}", entry);
        serial_println!("[sprout] Stack pointer: {:#x}", stack);
        serial_println!("[sprout] Segment selectors: cs=0x1B, ds=0x23");

        // Double-check that memory is mapped where we think it is
        unsafe {
            let test_entry = *(entry as *const u8);
            serial_println!("[sprout] Entry memory first byte: {:#x}", test_entry);
        }

        serial_println!("[sprout] About to iretq...");

        unsafe {
            asm!(
                "cli",
                "mov ax, 0x23",
                "mov ds, ax",
                "mov es, ax",
                "mov fs, ax",
                "mov gs, ax",

                "push 0x23",        // SS
                "push {stk}",       // RSP
                "pushf",            // RFLAGS
                "pop rax",
                "or rax, 0x200",    // set IF
                "push rax",
                "push 0x1B",        // CS
                "push {e}",         // RIP
                "iretq",
                stk = in(reg) stack,
                e   = in(reg) entry,
                options(noreturn)
            );

            serial_println!("user wrote: {:#x}", unsafe { *(0x500000 as *const u64) });
        }
    }
}
