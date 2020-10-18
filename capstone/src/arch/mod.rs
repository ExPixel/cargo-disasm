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

impl InsnId {
    #[inline]
    pub(crate) fn to_c(self) -> libc::c_int {
        match self {
            InsnId::X86(id) => id.to_c(),
        }
    }
}

impl From<x86::InsnId> for InsnId {
    #[inline]
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

macro_rules! impl_reg_conversions {
    ($Arch:ident, $RegType:ty) => {
        impl PartialEq<$RegType> for GenericReg {
            #[inline]
            fn eq(&self, other: &$RegType) -> bool {
                self.0 == other.to_primitive() as u16
            }
        }

        impl PartialEq<GenericReg> for $RegType {
            #[inline]
            fn eq(&self, other: &GenericReg) -> bool {
                self.to_primitive() as u16 == other.0
            }
        }

        impl core::convert::From<$RegType> for GenericReg {
            #[inline]
            fn from(arch_reg: $RegType) -> Self {
                GenericReg(arch_reg.to_primitive() as u16)
            }
        }

        impl core::convert::From<GenericReg> for $RegType {
            #[inline]
            fn from(generic: GenericReg) -> $RegType {
                <$RegType>::from_c(generic.0 as libc::c_int).unwrap_or(<$RegType>::Invalid)
            }
        }

        impl GenericReg {
            /// Convert a generic register to an architecture specific register.
            #[inline]
            pub fn $Arch(self) -> $RegType {
                <$RegType>::from_c(self.0 as libc::c_int).unwrap_or(<$RegType>::Invalid)
            }
        }
    };
}

impl_reg_conversions!(x86, x86::Reg);
