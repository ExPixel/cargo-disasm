#[repr(C)]
#[derive(Clone, Copy)]
pub struct Details {
    /// Instruction prefix, which can be up to 4 bytes.
    /// A prefix byte gets value 0 when irrelevant.
    /// prefix[0] indicates REP/REPNE/LOCK prefix (See X86_PREFIX_REP/REPNE/LOCK above)
    /// prefix[1] indicates segment override (irrelevant for x86_64):
    /// See X86_PREFIX_CS/SS/DS/ES/FS/GS above.
    /// prefix[2] indicates operand-size override (X86_PREFIX_OPSIZE)
    /// prefix[3] indicates address-size override (X86_PREFIX_ADDRSIZE)
    prefix: [u8; 4],

    /// Instruction opcode, which can be from 1 to 4 bytes in size.
    /// This contains VEX opcode as well.
    /// An trailing opcode byte gets value 0 when irrelevant.
    opcode: [u8; 4],

    /// REX prefix: only a non-zero value is relevant for x86_64
    rex: u8,

    /// Address size, which can be overridden with above prefix[5].
    addr_size: u8,

    /// ModR/M byte
    modrm: u8,

    /// SIB value, or 0 when irrelevant.
    sib: u8,

    /// Displacement value, valid if encoding.disp_offset != 0
    disp: u64,

    /// SIB index register, or X86_REG_INVALID when irrelevant.
    sib_index: X86Reg,
    /// SIB scale, only applicable if sib_index is valid.
    sib_scale: libc::c_int,
    /// SIB base register, or X86_REG_INVALID when irrelevant.
    sib_base: X86Reg,

    /// XOP Code Condition
    xop_cc: X86XopCC,
    /// SSE Code Condition
    sse_cc: X86SseCC,
    /// AVX Code Condition
    avx_cc: X86AvxCC,

    /// AVX suppress all exceptions
    avx_sae: bool,
    /// AVX static rounding mode
    avx_rm: X86AvxRm,

    eflags_or_fpu_flags: X86EFlagsOrFpuFlags,

    /// Number of operands of this instruction,
    /// or 0 when instruction has no operands.
    op_count: u8,

    /// Operands for this instruction.
    operands: [Op; 8],

    /// Encoding information
    encoding: Encoding,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Op {
    type_: X86OpType,
    value: X86OpValue,

    /// Size of this operand (in bytes).
    size: u8,

    /// How this operand is accessed. (READ, WRITE, READ | WRITE)
    /// This field is a combination of cs_ac_type.
    /// NOTE: this field is irrelevant if the engine is compiled in DIET mode.
    access: u8,

    /// AVX broadcast type, or 0 if irrelevant.
    avx_bcast: X86AvxBCast,

    /// AVX zero opmask {Z}
    avx_zero_opmask: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Encoding {
    /// ModR/M offset, or 0 when irrelevant.
    modrm_offset: u8,

    /// Displacement offset, or 0 when irrelevant.
    disp_offset: u8,
    disp_size: u8,

    /// Immediate offset, or 0 when irrelevant.
    imm_offset: u8,
    imm_size: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpMem {
    /// Segment register
    segment: X86Reg,
    /// Base register
    base: X86Reg,
    /// Index register
    index: X86Reg,
    /// Scale for index register
    scale: libc::c_int,
    /// Displacement value
    disp: u64,
}

c_enum_big! {
    #[non_exhaustive]
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub enum Reg: u8 + i32 + u32 {
        @Start = Invalid,
        @End   = Ending,

        Invalid = 0,
        AH,
        AL,
        AX,
        BH,
        BL,
        BP,
        BPL,
        BX,
        CH,
        CL,
        CS,
        CX,
        DH,
        DI,
        DIL,
        DL,
        DS,
        DX,
        EAX,
        EBP,
        EBX,
        ECX,
        EDI,
        EDX,
        EFLAGS,
        EIP,
        EIZ,
        ES,
        ESI,
        ESP,
        FPSW,
        FS,
        GS,
        IP,
        RAX,
        RBP,
        RBX,
        RCX,
        RDI,
        RDX,
        RIP,
        RIZ,
        RSI,
        RSP,
        SI,
        SIL,
        SP,
        SPL,
        SS,
        CR0,
        CR1,
        CR2,
        CR3,
        CR4,
        CR5,
        CR6,
        CR7,
        CR8,
        CR9,
        CR10,
        CR11,
        CR12,
        CR13,
        CR14,
        CR15,
        DR0,
        DR1,
        DR2,
        DR3,
        DR4,
        DR5,
        DR6,
        DR7,
        DR8,
        DR9,
        DR10,
        DR11,
        DR12,
        DR13,
        DR14,
        DR15,
        FP0,
        FP1,
        FP2,
        FP3,
        FP4,
        FP5,
        FP6,
        FP7,
        K0,
        K1,
        K2,
        K3,
        K4,
        K5,
        K6,
        K7,
        MM0,
        MM1,
        MM2,
        MM3,
        MM4,
        MM5,
        MM6,
        MM7,
        R8,
        R9,
        R10,
        R11,
        R12,
        R13,
        R14,
        R15,
        ST0,
        ST1,
        ST2,
        ST3,
        ST4,
        ST5,
        ST6,
        ST7,
        XMM0,
        XMM1,
        XMM2,
        XMM3,
        XMM4,
        XMM5,
        XMM6,
        XMM7,
        XMM8,
        XMM9,
        XMM10,
        XMM11,
        XMM12,
        XMM13,
        XMM14,
        XMM15,
        XMM16,
        XMM17,
        XMM18,
        XMM19,
        XMM20,
        XMM21,
        XMM22,
        XMM23,
        XMM24,
        XMM25,
        XMM26,
        XMM27,
        XMM28,
        XMM29,
        XMM30,
        XMM31,
        YMM0,
        YMM1,
        YMM2,
        YMM3,
        YMM4,
        YMM5,
        YMM6,
        YMM7,
        YMM8,
        YMM9,
        YMM10,
        YMM11,
        YMM12,
        YMM13,
        YMM14,
        YMM15,
        YMM16,
        YMM17,
        YMM18,
        YMM19,
        YMM20,
        YMM21,
        YMM22,
        YMM23,
        YMM24,
        YMM25,
        YMM26,
        YMM27,
        YMM28,
        YMM29,
        YMM30,
        YMM31,
        ZMM0,
        ZMM1,
        ZMM2,
        ZMM3,
        ZMM4,
        ZMM5,
        ZMM6,
        ZMM7,
        ZMM8,
        ZMM9,
        ZMM10,
        ZMM11,
        ZMM12,
        ZMM13,
        ZMM14,
        ZMM15,
        ZMM16,
        ZMM17,
        ZMM18,
        ZMM19,
        ZMM20,
        ZMM21,
        ZMM22,
        ZMM23,
        ZMM24,
        ZMM25,
        ZMM26,
        ZMM27,
        ZMM28,
        ZMM29,
        ZMM30,
        ZMM31,
        R8B,
        R9B,
        R10B,
        R11B,
        R12B,
        R13B,
        R14B,
        R15B,
        R8D,
        R9D,
        R10D,
        R11D,
        R12D,
        R13D,
        R14D,
        R15D,
        R8W,
        R9W,
        R10W,
        R11W,
        R12W,
        R13W,
        R14W,
        R15W,

        #[doc(hidden)]
        Ending,
    }
}

c_enum! {
    /// Operand type for an x86 instruction's operands.
    pub enum OpType: u8 + i32 + u32 {
        /// Uninitialized.
        Invalid = 0,
        /// Register operand.
        Reg,
        /// Immediate operand.
        Imm,
        /// Memory operand.
        Mem,
    }
}

c_enum! {
    /// XOP Code Condition Type.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum XopCC: u8 + i32 + u32 {
        /// Uninitialized.
        Invalid = 0,
        Lt,
        Le,
        Get,
        Ge,
        Eq,
        Neq,
        False,
        True,
    }
}

c_enum! {
    /// AXV broadcast type.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum AvxBroadcast: u8 {
        Invalid = 0,
        /// AVX 512 broadcast type {1to2}
        To2,
        /// AVX 512 broadcast type {1to4}
        To4,
        /// AVX 512 broadcast type {1to8}
        To8,
        /// AVX 512 broadcast type {1to16}
        To16,
    }
}

c_enum! {
    pub enum SseCC: u8 {
        Placeholder
    }
}

/// x86_reg
type X86Reg = libc::c_int;
/// x86_xop_cc
type X86XopCC = libc::c_int;
/// x86_sse_cc
type X86SseCC = libc::c_int;
/// x86_avc_cc
type X86AvxCC = libc::c_int;
/// x86_avx_rm
type X86AvxRm = libc::c_int;
/// x86_op_type
type X86OpType = libc::c_int;
/// x86_avx_bast
type X86AvxBCast = libc::c_int;

#[repr(C)]
#[derive(Clone, Copy)]
union X86EFlagsOrFpuFlags {
    /// EFLAGS updated by an instruction.
    /// This can be from an OR combination of X86_EFLAGS_* symbols
    eflags: u64,
    /// FPU_FLAGS updated by an instruction.
    /// This can be formed from an OR combination of X86_FPU_FLAGS_*
    fpu_flags: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
union X86OpValue {
    reg: Reg,
    imm: u64,
    mem: OpMem,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn x86_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Details>(), unsafe {
            sys::ep_helper__sizeof_cs_x86() as usize
        });

        assert_eq!(core::mem::align_of::<Details>(), unsafe {
            sys::ep_helper__alignof_cs_x86() as usize
        });
    }
}
