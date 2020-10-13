#[inline]
pub unsafe fn cstr(ptr: *const libc::c_char, max_size: usize) -> &'static str {
    let mut len = 0;

    // strlen:
    while len < max_size {
        if ptr.add(len).read() == 0 {
            break;
        } else {
            len += 1;
        }
    }

    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, len))
}
