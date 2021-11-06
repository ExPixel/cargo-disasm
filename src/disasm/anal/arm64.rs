use super::Jump;
use capstone::{arm64, Capstone, Insn};

pub fn identify_jump_target(insn: &Insn, caps: &Capstone) -> Jump {
    let generic_details = caps.details(insn);

    let is_jump = generic_details.groups().iter().any(|&g| {
        g == arm64::InsnGroup::Call
            || g == arm64::InsnGroup::Jump
            || g == arm64::InsnGroup::BranchRelative
    });

    if !is_jump {
        return Jump::None;
    }

    if let Some(details) = generic_details.arm64() {
        if details.op_count() != 1 {
            return Jump::None;
        }

        match details.operands()[0].value() {
            arm64::OpValue::Imm(addr) => Jump::External(addr as u64),
            _ => Jump::None,
        }
    } else {
        log::error!("instruction did not have arm64 details");
        Jump::None
    }
}
