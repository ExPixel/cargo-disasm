use super::Jump;
use capstone::{x86, Capstone, Insn};

pub fn identify_jump_target(insn: &Insn, caps: &Capstone) -> Jump {
    let generic_details = caps.details(insn);

    let is_jump = generic_details.groups().iter().any(|&g| {
        g == x86::InsnGroup::Call
            || g == x86::InsnGroup::Jump
            || g == x86::InsnGroup::BranchRelative
    });

    if !is_jump {
        return Jump::None;
    }

    if let Some(details) = generic_details.x86() {
        // Do these even exist?
        if details.operands().len() != 1 {
            return Jump::None;
        }

        match details.operands()[0].value() {
            x86::OpValue::Imm(addr) => Jump::External(addr as u64),
            _ => Jump::None,
        }
    } else {
        log::error!("instruction did not have x86 details");
        Jump::None
    }
}
