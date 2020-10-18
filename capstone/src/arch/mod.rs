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

#[derive(Copy, Clone)]
pub enum InsnId {
    X86(x86::InsnId),
}

impl core::convert::From<InsnId> for libc::c_int {
    fn from(id: InsnId) -> Self {
        match id {
            InsnId::X86(id) => id.into(),
        }
    }
}

impl From<x86::InsnId> for InsnId {
    fn from(id: x86::InsnId) -> InsnId {
        InsnId::X86(id)
    }
}
