/// Wrapper around cs_x86
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct X86DetailsInner {
    pub x: libc::c_int,
}
