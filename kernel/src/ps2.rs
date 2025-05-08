pub fn enable_ps2_devices() {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut cmd_port = Port::<u8>::new(0x64);
        let mut data_port = Port::<u8>::new(0x60);

        // Enable PS/2 port 1 (keyboard)
        cmd_port.write(0xAE);

        // Enable PS/2 port 2 (mouse)
        cmd_port.write(0xA8);

        // Read the current command byte
        cmd_port.write(0x20);
        let mut status: u8 = data_port.read();

        // Set bits to enable IRQ1 (bit 0) and IRQ12 (bit 1)
        status |= 0b11;

        // Write command byte back
        cmd_port.write(0x60);
        data_port.write(status);

        log::info!("PS/2 controller enabled (IRQ1 and IRQ12).");
    }
}
