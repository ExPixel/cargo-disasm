mod arm64;
mod x86;

use capstone::{Arch, Capstone, Insn};

pub fn identify_jump_target(insn: &Insn, caps: &Capstone) -> Jump {
    match caps.arch() {
        Arch::Arm64 => arm64::identify_jump_target(insn, caps),
        Arch::X86 => x86::identify_jump_target(insn, caps),
        _ => Jump::None,
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Jump {
    /// This is a jump an internal instruction inside of the symbol's function.
    Internal(usize),
    /// This is a jump to some external address that should be symbolicated.
    External(u64),
    /// There is no jump.
    None,
}

impl Jump {
    #[inline]
    pub fn is_internal(&self) -> bool {
        matches!(self, &Jump::Internal(..))
    }

    #[inline]
    pub fn is_external(&self) -> bool {
        matches!(self, &Jump::External(..))
    }
}
