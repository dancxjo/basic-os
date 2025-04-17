#[allow(static_mut_refs)]
use alloc::boxed::Box;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, DS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub struct Selectors {
    pub code_ker: SegmentSelector,
    pub data_ker: SegmentSelector,
    pub code_usr: SegmentSelector,
    pub data_usr: SegmentSelector,
    pub tss: SegmentSelector,
}

// Global GDT and TSS pointers (leaked for 'static use)
static mut GDT: Option<&'static mut GlobalDescriptorTable> = None;
pub static mut TSS: Option<&'static mut TaskStateSegment> = None;
pub static mut SELECTORS: Option<Selectors> = None;

pub fn init_gdt() {
    // 1. Leak GDT and TSS into static memory
    let gdt: &'static mut GlobalDescriptorTable = Box::leak(Box::new(GlobalDescriptorTable::new()));
    let tss: &'static mut TaskStateSegment = Box::leak(Box::new(TaskStateSegment::new()));
    // 2. Set up IST for double fault
    let df_stack_top = 0x4444_7000_0000 + 5 * 4096;
    tss.interrupt_stack_table[0] = VirtAddr::new(df_stack_top);

    // 3. Add segments
    let code_ker = gdt.add_entry(Descriptor::kernel_code_segment());
    let data_ker = gdt.add_entry(Descriptor::kernel_data_segment());
    let code_usr = gdt.add_entry(Descriptor::user_code_segment());
    let data_usr = gdt.add_entry(Descriptor::user_data_segment());
    let tss_sel = {
        let tss_ref: &'static TaskStateSegment = unsafe { &*(tss as *const _) };
        gdt.add_entry(Descriptor::tss_segment(tss_ref))
    };

    unsafe {
        GDT = Some(&mut *(gdt as *mut _));
        TSS = Some(&mut *(tss as *mut _));

        gdt.load();
        CS::set_reg(code_ker);
        DS::set_reg(data_ker);
        load_tss(tss_sel);

        SELECTORS = Some(Selectors {
            code_ker,
            data_ker,
            code_usr,
            data_usr,
            tss: tss_sel,
        });
    }
}
