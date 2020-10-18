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

use core::cmp::{Eq, PartialEq};

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

/// A generic register that can be compared to any architecture specific register.
/// This register may be equal to multiple registers from different architectures
/// but not to multiple registers of the same architecture. This can also be convered
/// to an architecture specific register for any architecture.
#[derive(Copy, Clone, PartialEq, Eq, Default, Hash)]
#[repr(transparent)]
pub struct GenericReg(u16);
