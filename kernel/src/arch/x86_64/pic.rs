use x86_64::instructions::port::Port;

/// PIC I/O Ports
const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

/// Interrupt Vector Offsets
const PIC1_OFFSET: u8 = 32; // IRQ0..7 -> interrupt 32..39
const PIC2_OFFSET: u8 = 40; // IRQ8..15 -> interrupt 40..47

/// Initialize the PICs: remap and set masks
pub unsafe fn init_pic() {
    let mut pic1_cmd = Port::<u8>::new(PIC1_CMD);
    let mut pic1_data = Port::<u8>::new(PIC1_DATA);
    let mut pic2_cmd = Port::<u8>::new(PIC2_CMD);
    let mut pic2_data = Port::<u8>::new(PIC2_DATA);
    unsafe {
        // Begin initialization (ICW1)
        pic1_cmd.write(0x11);
        pic2_cmd.write(0x11);

        // Set vector offsets (ICW2)
        pic1_data.write(PIC1_OFFSET);
        pic2_data.write(PIC2_OFFSET);

        // Tell Master PIC about Slave PIC at IRQ2 (ICW3)
        pic1_data.write(0x04);
        // Tell Slave PIC its cascade identity (ICW3)
        pic2_data.write(0x02);

        // Set mode to 8086/88 (ICW4)
        pic1_data.write(0x01);
        pic2_data.write(0x01);

        // Allow IRQ0 (timer) and IRQ1 (keyboard)
        pic1_data.write(0b1111_1100);
        pic2_data.write(0b1111_1111);
    }
    log::info!("PIC remapped and initialized.");
}

/// Send End-of-Interrupt (EOI) to Master PIC
pub fn pic_end_of_interrupt(irq: u8) {
    unsafe {
        if irq >= 8 {
            let mut slave = Port::<u8>::new(PIC2_CMD);
            slave.write(0x20);
        }
        let mut master = Port::<u8>::new(PIC1_CMD);
        master.write(0x20);
    }
}
