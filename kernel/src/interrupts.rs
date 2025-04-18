use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::instructions::interrupts;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init_interrupts() {
    unsafe { PICS.lock().initialize() };
    interrupts::enable();
}
