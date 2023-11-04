use super::generated::{cs_arm64, cs_arm64_op, arm64_op_mem};
use core::marker::PhantomData;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Details<'c> {
    inner: cs_arm64,
    _phantom: PhantomData<&'c ()>,
}

impl<'c> Details<'c> {
    /// Returns the number of operands in this instruction, or
    /// zero when this instruction has no operands. This value will
    /// be the same as the length of the slice returned by [`Details::operands`].
    pub fn op_count(&self) -> usize {
        self.inner.op_count as usize
    }

    /// Returns the operands contained in this instruction. The length
    /// of the returned slice will be the same as the value returned
    /// by [`Details::op_count`].
    pub fn operands(&self) -> &[Op] {
        unsafe {
            &*(&self.inner.operands[..self.inner.op_count as usize] as *const [cs_arm64_op]
                as *const [Op])
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Op {
    inner: cs_arm64_op,
}

impl Op {
    /// Returns the type of this operand.
    pub fn op_type(&self) -> OpType {
        OpType::from_c(self.inner.type_).unwrap_or(OpType::Invalid)
    }

    /// Returns the value of this operand.
    pub fn value(&self) -> OpValue {
        match self.op_type() {
            OpType::Invalid => OpValue::Imm(0),
            OpType::Reg => OpValue::Reg(
                Reg::from_c(unsafe { self.inner.__bindgen_anon_1.reg }).unwrap_or(Reg::Invalid),
            ),
            OpType::Imm => OpValue::Imm(unsafe { self.inner.__bindgen_anon_1.imm }),
            OpType::Mem => OpValue::Mem(unsafe {
                OpMem {
                    inner: self.inner.__bindgen_anon_1.mem,
                }
            }),
            OpType::Fp => OpValue::Fp(unsafe { self.inner.__bindgen_anon_1.fp }),
        }
    }
}

pub enum OpValue {
    Reg(Reg),
    Imm(i64),
    Fp(f64),
    Mem(OpMem),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpMem {
    inner: arm64_op_mem,
}

c_enum! {
    /// Operand type for an arm64 instruction's operands.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum OpType: u8 {
        /// Uninitialized.
        Invalid = 0,
        /// Register operand.
        Reg,
        /// Immediate operand.
        Imm,
        /// Memory operand.
        Mem,
        /// Floating-Point operand.
        Fp,
    }
}

c_enum_big! {
    #[non_exhaustive]
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub enum Reg: u8 {
        @Start = Invalid,
        @End   = Ending,

        Invalid = 0,
        X29,
        X30,
        Nzcv,
        Sp,
        Wsp,
        Wzr,
        Xzr,
        B0,
        B1,
        B2,
        B3,
        B4,
        B5,
        B6,
        B7,
        B8,
        B9,
        B10,
        B11,
        B12,
        B13,
        B14,
        B15,
        B16,
        B17,
        B18,
        B19,
        B20,
        B21,
        B22,
        B23,
        B24,
        B25,
        B26,
        B27,
        B28,
        B29,
        B30,
        B31,
        D0,
        D1,
        D2,
        D3,
        D4,
        D5,
        D6,
        D7,
        D8,
        D9,
        D10,
        D11,
        D12,
        D13,
        D14,
        D15,
        D16,
        D17,
        D18,
        D19,
        D20,
        D21,
        D22,
        D23,
        D24,
        D25,
        D26,
        D27,
        D28,
        D29,
        D30,
        D31,
        H0,
        H1,
        H2,
        H3,
        H4,
        H5,
        H6,
        H7,
        H8,
        H9,
        H10,
        H11,
        H12,
        H13,
        H14,
        H15,
        H16,
        H17,
        H18,
        H19,
        H20,
        H21,
        H22,
        H23,
        H24,
        H25,
        H26,
        H27,
        H28,
        H29,
        H30,
        H31,
        Q0,
        Q1,
        Q2,
        Q3,
        Q4,
        Q5,
        Q6,
        Q7,
        Q8,
        Q9,
        Q10,
        Q11,
        Q12,
        Q13,
        Q14,
        Q15,
        Q16,
        Q17,
        Q18,
        Q19,
        Q20,
        Q21,
        Q22,
        Q23,
        Q24,
        Q25,
        Q26,
        Q27,
        Q28,
        Q29,
        Q30,
        Q31,
        S0,
        S1,
        S2,
        S3,
        S4,
        S5,
        S6,
        S7,
        S8,
        S9,
        S10,
        S11,
        S12,
        S13,
        S14,
        S15,
        S16,
        S17,
        S18,
        S19,
        S20,
        S21,
        S22,
        S23,
        S24,
        S25,
        S26,
        S27,
        S28,
        S29,
        S30,
        S31,
        W0,
        W1,
        W2,
        W3,
        W4,
        W5,
        W6,
        W7,
        W8,
        W9,
        W10,
        W11,
        W12,
        W13,
        W14,
        W15,
        W16,
        W17,
        W18,
        W19,
        W20,
        W21,
        W22,
        W23,
        W24,
        W25,
        W26,
        W27,
        W28,
        W29,
        W30,
        X0,
        X1,
        X2,
        X3,
        X4,
        X5,
        X6,
        X7,
        X8,
        X9,
        X10,
        X11,
        X12,
        X13,
        X14,
        X15,
        X16,
        X17,
        X18,
        X19,
        X20,
        X21,
        X22,
        X23,
        X24,
        X25,
        X26,
        X27,
        X28,

        #[doc(hidden)]
        Ending,
    }
}

c_enum_big! {
    #[non_exhaustive]
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub enum InsnGroup: u8 {
        @Start = Invalid,
        @End   = Ending,

        Invalid = 0,

        // Generic groups
        /// All jump instructions (conditional+direct+indirect jumps)
        Jump,
        /// All call instructions
        Call,
        /// All return instructions
        Ret,
        /// All interrupt instructions
        Int,
        /// All privileged instructions
        Privilege = 6,
        /// All relative branching instructions
        BranchRelative,

        Ending,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn arm64_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_arm64)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
            sys::get_test_val("alignof(cs_arm64)")
        );
    }
}
