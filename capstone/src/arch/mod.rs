pub mod arm;
pub mod arm64;
pub mod evm;
pub mod m680x;
pub mod m68k;
pub mod mips;
pub mod mos65xx;
pub mod ppc;
pub mod sparc;
pub mod sysz;
pub mod tms320c64x;
pub mod x86;
pub mod xcore;

pub enum InsnId {}

impl core::convert::From<InsnId> for libc::c_int {
    fn from(_id: InsnId) -> Self {
        // FIXME implement this
        0
    }
}
