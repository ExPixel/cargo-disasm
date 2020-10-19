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

bitflags::bitflags! {
    /// Common instruction operand access types.
    #[repr(transparent)]
    pub struct Access: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
    }
}

/// A generic instruction ID that can be compared to any architecture specific
/// instruction ID. Unlike [`InsnGroup`] and [`Reg`], this generic instruction ID
/// can only be equal to one instruction ID from one architecture.
#[derive(Copy, Clone, Hash, PartialEq, Eq)]
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

/// A generic group that can be compared to any architecture specific group.
/// This group may be equal to multiple groups from different architectures but
/// not to multiple groups from the same architecture. This can also be converted
/// into an architecture specific group for any architecture.
#[derive(Copy, Clone, PartialEq, Eq, Default, Hash)]
#[repr(transparent)]
pub struct InsnGroup(u16);

impl InsnGroup {
    pub(crate) fn to_primitive(self) -> u16 {
        self.0
    }
}

/// A generic register that can be compared to any architecture specific register.
/// This register may be equal to multiple registers from different architectures
/// but not to multiple registers of the same architecture. This can also be converted
/// to an architecture specific register for any architecture.
#[derive(Copy, Clone, PartialEq, Eq, Default, Hash)]
#[repr(transparent)]
pub struct Reg(u16);

impl Reg {
    pub(crate) fn to_primitive(self) -> u16 {
        self.0
    }
}

macro_rules! impl_arch {
    ($ArchModuleName:ident, $ArchTypeName:ident, $ArchFnName:ident) => {
        impl From<$ArchModuleName::InsnId> for InsnId {
            #[inline]
            fn from(id: $ArchModuleName::InsnId) -> InsnId {
                InsnId::$ArchTypeName(id)
            }
        }

        impl PartialEq<$ArchModuleName::InsnId> for InsnId {
            #[inline]
            fn eq(&self, other: &$ArchModuleName::InsnId) -> bool {
                matches!(self, InsnId::$ArchTypeName(inner) if inner == other)
            }
        }

        impl PartialEq<$ArchModuleName::InsnGroup> for InsnGroup {
            #[inline]
            fn eq(&self, other: &$ArchModuleName::InsnGroup) -> bool {
                self.0 == other.to_primitive() as u16
            }
        }

        impl PartialEq<InsnGroup> for $ArchModuleName::InsnGroup {
            #[inline]
            fn eq(&self, other: &InsnGroup) -> bool {
                self.to_primitive() as u16 == other.0
            }
        }

        impl core::convert::From<$ArchModuleName::InsnGroup> for InsnGroup {
            #[inline]
            fn from(arch_insn_group: $ArchModuleName::InsnGroup) -> Self {
                InsnGroup(arch_insn_group.to_primitive() as u16)
            }
        }

        impl core::convert::From<InsnGroup> for $ArchModuleName::InsnGroup {
            #[inline]
            fn from(generic: InsnGroup) -> $ArchModuleName::InsnGroup {
                $ArchModuleName::InsnGroup::from_c(generic.0 as libc::c_int)
                    .unwrap_or($ArchModuleName::InsnGroup::Invalid)
            }
        }

        impl InsnGroup {
            /// Convert a generic instruction group to an architecture specific instruction group.
            #[inline]
            pub fn $ArchFnName(self) -> $ArchModuleName::InsnGroup {
                $ArchModuleName::InsnGroup::from_c(self.0 as libc::c_int)
                    .unwrap_or($ArchModuleName::InsnGroup::Invalid)
            }
        }

        impl PartialEq<$ArchModuleName::Reg> for Reg {
            #[inline]
            fn eq(&self, other: &$ArchModuleName::Reg) -> bool {
                self.0 == other.to_primitive() as u16
            }
        }

        impl PartialEq<Reg> for $ArchModuleName::Reg {
            #[inline]
            fn eq(&self, other: &Reg) -> bool {
                self.to_primitive() as u16 == other.0
            }
        }

        impl core::convert::From<$ArchModuleName::Reg> for Reg {
            #[inline]
            fn from(arch_reg: $ArchModuleName::Reg) -> Self {
                Reg(arch_reg.to_primitive() as u16)
            }
        }

        impl core::convert::From<Reg> for $ArchModuleName::Reg {
            #[inline]
            fn from(generic: Reg) -> $ArchModuleName::Reg {
                $ArchModuleName::Reg::from_c(generic.0 as libc::c_int)
                    .unwrap_or($ArchModuleName::Reg::Invalid)
            }
        }

        impl Reg {
            /// Convert a generic register to an architecture specific register.
            #[inline]
            pub fn $ArchFnName(self) -> $ArchModuleName::Reg {
                $ArchModuleName::Reg::from_c(self.0 as libc::c_int)
                    .unwrap_or($ArchModuleName::Reg::Invalid)
            }
        }
    };
}

impl_arch!(x86, X86, x86);
