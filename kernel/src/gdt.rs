use x86_64::{
    VirtAddr,
    instructions::segmentation::{CS, DS, SS, Segment},
    instructions::tables::load_tss,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

// Place for the TSS and GDT
static mut GDT: Option<GlobalDescriptorTable> = None;
static mut TSS: Option<TaskStateSegment> = None;

pub struct Selectors {
    pub code: SegmentSelector,
    pub data: SegmentSelector,
    pub tss: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
}

pub static mut SELECTORS: Option<Selectors> = None;

pub fn init_gdt() {
    unsafe {
        // --- Create TSS ---
        TSS = Some(TaskStateSegment::new());
        #[allow(static_mut_refs)]
        let tss = TSS.as_mut().unwrap();

        // Set up stack pointers for exceptions and privilege level transitions
        let ist_base = 0x4444_7000_0000u64;
        tss.interrupt_stack_table[0] = VirtAddr::new(ist_base + 8 * 4096); // IST[0] = double fault
        tss.interrupt_stack_table[1] = VirtAddr::new(ist_base + 6 * 4096); // IST[1] = timer

        tss.privilege_stack_table[0] = VirtAddr::new(ist_base + 4 * 4096); // Ring 0 stack (SS0)

        // --- Create GDT ---
        let mut gdt = GlobalDescriptorTable::new();

        // Kernel segments
        let code_sel = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_sel = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(tss));
        let user_data_sel = gdt.add_entry(Descriptor::UserSegment(0x00af_9200_0000_0000));
        let user_code_sel = gdt.add_entry(Descriptor::UserSegment(0x00af_9a00_0000_0000));

        // Store it globally
        GDT = Some(gdt);
        SELECTORS = Some(Selectors {
            code: code_sel,
            data: data_sel,
            tss: tss_sel,
            user_code: user_code_sel,
            user_data: user_data_sel,
        });

        // --- Load GDT ---
        #[allow(static_mut_refs)]
        GDT.as_ref().unwrap().load();

        // --- Reload segment registers ---
        CS::set_reg(code_sel);
        DS::set_reg(data_sel);
        SS::set_reg(data_sel);

        // (Optional) Set ES/FS/GS if needed

        // --- Load TSS ---
        load_tss(tss_sel);
    }
}
