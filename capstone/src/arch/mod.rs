pub mod x86;

pub enum InsnId {}

impl core::convert::From<InsnId> for libc::c_int {
    fn from(_id: InsnId) -> Self {
        // FIXME implement this
        0
    }
}
