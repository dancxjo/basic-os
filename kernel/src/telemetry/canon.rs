//! Symbol table helpers. Keep codes short and human-readable for dumps.

/// Combine two ASCII bytes into a human-readable 16-bit code.
pub const fn cc(a: u8, b: u8) -> u16 {
    ((a as u16) << 8) | (b as u16)
}

// Common symbols used by early drivers and journal dumps.
pub const JOURNAL: u16 = cc(b'J', b'N');
pub const KEYBOARD: u16 = cc(b'K', b'B');
pub const MOUSE: u16 = cc(b'M', b'S');
pub const PRESSED: u16 = cc(b'P', b'R');
pub const MOVED: u16 = cc(b'M', b'V');
pub const AT: u16 = cc(b'@', b' ');

/// Return the symbolic name for a code if known.
pub fn sym_name(code: u16) -> &'static str {
    // Linear scan keeps it tiny; expand or sort if this grows.
    const TBL: &[(u16, &str)] = &[
        (JOURNAL, "journal"),
        (KEYBOARD, "keyboard"),
        (MOUSE, "mouse"),
        (PRESSED, "pressed"),
        (MOVED, "moved"),
        (AT, "at"),
    ];
    let mut i = 0;
    while i < TBL.len() {
        if TBL[i].0 == code {
            return TBL[i].1;
        }
        i += 1;
    }
    ""
}
