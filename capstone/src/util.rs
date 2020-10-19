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

/// Ensures that a string is 0 terminated.
#[cfg(feature = "alloc")]
pub fn ensure_c_string(
    s: crate::alloc::borrow::Cow<'static, str>,
) -> crate::alloc::borrow::Cow<'static, str> {
    if s.ends_with('\0') {
        return s;
    }
    let mut s = s.into_owned();
    s.push('\0');
    s.into()
}

#[cfg(not(feature = "alloc"))]
pub fn ensure_c_string(s: &'static str) -> &'static str {
    assert!(s.ends_with('\0'), "not a valid 0 terminated string");
    s
}
