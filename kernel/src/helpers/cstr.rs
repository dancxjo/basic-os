pub unsafe fn cstr_to_str(ptr: *const u8) -> &'static str {
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }

    unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len)) }
}
