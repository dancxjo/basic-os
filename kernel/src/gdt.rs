use alloc::boxed::Box;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, DS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

struct Selectors {
    code_ker: SegmentSelector,
    data_ker: SegmentSelector,
    code_usr: SegmentSelector,
    data_usr: SegmentSelector,
    tss: SegmentSelector,
}

// Store references, not by-value
static mut GDT: Option<&'static mut GlobalDescriptorTable> = None;
static mut TSS: Option<&'static mut TaskStateSegment> = None;
static mut SELECTORS: Option<Selectors> = None;

pub fn init_gdt() {
    // 1) Leak them right away, so they're 'static from the start:
    let gdt_ref: &'static mut GlobalDescriptorTable =
        Box::leak(Box::new(GlobalDescriptorTable::new()));
    let tss_ref: &'static mut TaskStateSegment = Box::leak(Box::new(TaskStateSegment::new()));

    // 2) Store them in statics. This ensures the CPU can access them forever.
    unsafe {
        GDT = Some(gdt_ref);
        TSS = Some(tss_ref);
    }
    // 3) Now use those same references for your descriptor config:
    #[allow(static_mut_refs)]
    let gdt = unsafe { GDT.as_mut().unwrap() };
    #[allow(static_mut_refs)]
    let tss = unsafe { TSS.as_mut().unwrap() };

    // e.g. tss.interrupt_stack_table[0] = VirtAddr::new(some_stack_top);
    let double_fault_stack_top: u64 = 0x4444_7000_0000 + 4096 * 5;
    tss.interrupt_stack_table[0] = VirtAddr::new(double_fault_stack_top);

    // Kernel segments
    let code_ker = gdt.add_entry(Descriptor::kernel_code_segment());
    let data_ker = gdt.add_entry(Descriptor::kernel_data_segment());

    // User segments
    let code_usr = gdt.add_entry(Descriptor::user_code_segment());
    let data_usr = gdt.add_entry(Descriptor::user_data_segment());

    // TSS
    let tss_desc = Descriptor::tss_segment(tss);
    let tss_sel = gdt.add_entry(tss_desc);

    // Load GDT, set regs, TSS, etc.
    unsafe {
        gdt.load();
        CS::set_reg(code_ker);
        DS::set_reg(data_ker);
        load_tss(tss_sel);
    }

    // Save your selectors in a static as well
    unsafe {
        SELECTORS = Some(Selectors {
            code_ker,
            data_ker,
            code_usr,
            data_usr,
            tss: tss_sel,
        });
    }
}
