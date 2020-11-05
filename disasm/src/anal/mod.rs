mod x86;

use capstone::{Arch, Capstone, Insn};

pub fn identify_jump_target(insn: &Insn, caps: &Capstone) -> Jump {
    match caps.arch() {
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
