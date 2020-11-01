use super::generated::{cs_x86, cs_x86_encoding, cs_x86_op, x86_op_mem};
use core::marker::PhantomData;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Details<'c> {
    inner: cs_x86,
    _phantom: PhantomData<&'c ()>,
}

impl<'c> Details<'c> {
    /// Returns true if the instruction has the given prefix, or false otherwise.
    pub fn has_prefix(&self, mut prefix: Prefix) -> bool {
        if prefix == Prefix::RepE {
            prefix = Prefix::Rep
        }

        let idx = match prefix {
            Prefix::Lock | Prefix::Rep | Prefix::RepNE | Prefix::RepE => 0,
            Prefix::CS | Prefix::SS | Prefix::DS | Prefix::ES | Prefix::FS | Prefix::GS => 1,
            Prefix::OpSize => 2,
            Prefix::AddrSize => 3,
        };

        self.inner.prefix[idx] == prefix.to_primitive()
    }

    /// Instruction opcode. This value can be from 1 to 4 bytes in size.
    /// This will contain the VEX opcode as well.
    pub fn opcode(&self) -> &[u8] {
        let len = self.inner.opcode.iter().position(|&b| b == 0).unwrap_or(0);
        &self.inner.opcode[..len]
    }

    /// Returns the REX prefix byte. This value is only relevant
    /// for x86_64 and only if it is non-zero.
    pub fn rex(&self) -> u8 {
        self.inner.rex
    }

    /// Address size. This can be overriden by the [`Prefix::AddrSize`] prefix.
    pub fn addr_size(&self) -> u8 {
        self.inner.addr_size
    }

    /// Returns the ModR/M byte.
    pub fn modrm(&self) -> u8 {
        self.inner.modrm
    }

    /// Returns the SIB value. This will be zero if it is not relevant.
    pub fn sib(&self) -> u8 {
        self.inner.sib
    }

    /// Returns the displacement value. This is only valid if the value returned by [`Encoding::disp_offset`]
    /// which can be retrieved via [`Details::encoding`] is a non-zero value.
    pub fn disp(&self) -> i64 {
        self.inner.disp
    }

    /// Returns the SIB index register, or [`Reg::Invalid`] when irrelevant
    pub fn sib_index(&self) -> Reg {
        Reg::from_c(self.inner.sib_index).unwrap_or(Reg::Invalid)
    }

    /// Returns the SIB scale, only applicable if sib_index is valid.
    pub fn sib_scale(&self) -> i8 {
        self.inner.sib_scale
    }

    /// Returns the SIB base register, or [`Reg::Invalid`] when irrelevant.
    pub fn sib_base(&self) -> Reg {
        Reg::from_c(self.inner.sib_base).unwrap_or(Reg::Invalid)
    }

    /// Returns the XOP condition code.
    pub fn xop_cc(&self) -> XopCC {
        XopCC::from_c(self.inner.xop_cc).unwrap_or(XopCC::Invalid)
    }

    /// Returns the SSE condition code.
    pub fn sse_cc(&self) -> SseCC {
        SseCC::from_c(self.inner.sse_cc).unwrap_or(SseCC::Invalid)
    }

    /// Returns the AVX condition code.
    pub fn avx_cc(&self) -> AvxCC {
        AvxCC::from_c(self.inner.avx_cc).unwrap_or(AvxCC::Invalid)
    }

    /// Returns the AVX suppress all exceptions flag.
    pub fn avx_sae(&self) -> bool {
        self.inner.avx_sae
    }

    /// Returns the AVX static rounding mode.
    pub fn avx_rm(&self) -> AvxRm {
        AvxRm::from_c(self.inner.avx_rm).unwrap_or(AvxRm::Invalid)
    }

    /// Returns the number of operands in this instruction, or
    /// zero when this instruction has no operands. This value will
    /// be the same as the length of the slice returned by [`Details::operands`].
    pub fn op_count(&self) -> usize {
        self.inner.op_count as usize
    }

    /// Returns the operands contained in this instruction. The length
    /// of the returned slice will be the same as teh value returned
    /// by [`Details::op_count`].
    pub fn operands(&self) -> &[Op] {
        unsafe {
            &*(&self.inner.operands[..self.inner.op_count as usize] as *const [cs_x86_op]
                as *const [Op])
        }
    }

    /// Returns encoding information about this instruction.
    pub fn encoding(&self) -> &Encoding {
        unsafe { &*(&self.inner.encoding as *const cs_x86_encoding as *const Encoding) }
    }

    /// Returns the eflags updated by this instruction.
    /// This should not be called if the instruction is an FPU instruction,
    /// the return value will be undefined.
    pub fn eflags(&self) -> EFlags {
        EFlags::from_bits_truncate(unsafe { self.inner.__bindgen_anon_1.eflags })
    }

    /// Returns the FPU flags updated by this instruction.
    /// This should only be called if the instruction is
    /// in the FPU group, the return value will be undefined.
    pub fn fpu_flags(&self) -> FpuFlags {
        FpuFlags::from_bits_truncate(unsafe { self.inner.__bindgen_anon_1.fpu_flags })
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Op {
    inner: cs_x86_op,
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
        }
    }

    /// Returns the size of this operand in bytes.
    pub fn size(&self) -> usize {
        self.inner.size as usize
    }

    /// Returns how this operand was accessed.
    pub fn access(&self) -> super::Access {
        super::Access::from_bits_truncate(self.inner.access)
    }

    /// Returns AVX broadcast type, or [`AvxBroadcast::Invalid`] if irrelevant.
    pub fn avx_bcast(&self) -> AvxBroadcast {
        AvxBroadcast::from_c(self.inner.avx_bcast).unwrap_or(AvxBroadcast::Invalid)
    }

    /// Returns the AVX zero opmask {Z}
    pub fn avx_zero_opmask(&self) -> bool {
        self.inner.avx_zero_opmask
    }
}

pub enum OpValue {
    Reg(Reg),
    Imm(i64),
    Mem(OpMem),
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Encoding {
    inner: cs_x86_encoding,
}

impl Encoding {
    /// Returns the ModR/M offset, or 0 when irrelevant.
    pub fn modrm_offset(&self) -> u8 {
        self.inner.modrm_offset
    }

    /// Returns the displacement offset, or 0 when irrelevant.
    pub fn disp_offset(&self) -> u8 {
        self.inner.disp_offset
    }

    /// Returns the displacement size.
    pub fn disp_size(&self) -> u8 {
        self.inner.disp_size
    }

    /// Returns the immediate offset, or 0 when irrelevant.
    pub fn imm_offset(&self) -> u8 {
        self.inner.imm_offset
    }

    /// Returns the immediate size.
    pub fn imm_size(&self) -> u8 {
        self.inner.imm_size
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpMem {
    inner: x86_op_mem,
}

impl OpMem {
    /// Returns the segment register.
    pub fn segment(&self) -> Reg {
        Reg::from_c(self.inner.segment).unwrap_or(Reg::Invalid)
    }

    /// Returns the base register.
    pub fn base(&self) -> Reg {
        Reg::from_c(self.inner.base).unwrap_or(Reg::Invalid)
    }

    /// Returns the index register.
    pub fn index(&self) -> Reg {
        Reg::from_c(self.inner.index).unwrap_or(Reg::Invalid)
    }

    /// Returns the scale for the index register.
    pub fn scale(&self) -> i32 {
        self.inner.scale as i32
    }

    /// Returns the displacement value.
    pub fn disp(&self) -> i64 {
        self.inner.disp
    }
}

c_enum! {
    /// Instruction prefixes.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum Prefix: u8 {
        /// LOCK prefix
        Lock = 0xf0,
        /// REP prefix
        Rep = 0xf3,

        // NOTE: This is a special case (!!!).
        // It is swapped out with `Rep` when used.
        /// REPE/REPZ prefix
        RepE = 0x03,

        /// REPNE prefix
        RepNE = 0xf2,

        /// CS segment override
        CS = 0x2e,
        /// SS segment override
        SS = 0x36,
        /// DS segment override
        DS = 0x3e,
        /// ES segment override
        ES = 0x26,
        /// FS segment override
        FS = 0x64,
        /// GS segment override
        GS = 0x65,

        /// Operand size override
        OpSize = 0x66,

        /// Address size override
        AddrSize = 0x67,
    }
}

c_enum! {
    /// Operand type for an x86 instruction's operands.
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
    }
}

c_enum! {
    /// XOP Code Condition Type.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum XopCC: u8 {
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
    /// SSE condition codes.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum SseCC: u8 {
        Invalid = 0,
        Eq,
        Lt,
        Le,
        Unord,
        Neq,
        Nlt,
        Nle,
        Ord
    }
}

c_enum! {
    /// AVX condition codes.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum AvxCC: u8 {
        Invalid = 0,
        Eq,
        Lt,
        Le,
        Unord,
        Neq,
        Nlt,
        Nle,
        Ord,
        EqUq,
        Nge,
        Ngt,
        False,
        NeqOq,
        Ge,
        Gt,
        True,
        EqOs,
        LtOq,
        LeOq,
        UnordS,
        NeqUs,
        NltUq,
        NleUq,
        OrdS,
        EqUs,
        NgeUq,
        NgtUq,
        FalseOs,
        NeqOs,
        GeOq,
        GtOq,
        TrueUs,
    }
}

c_enum! {
    /// AVX rounding modes.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum AvxRm: u8 {
        Invalid = 0,
        /// Round to nearest.
        Rn,
        /// Round down.
        Rd,
        /// Round up.
        Ru,
        /// Round towards zero.
        Rz
    }
}

bitflags::bitflags! {
    pub struct EFlags: u64 {
        const MODIFY_AF = 1 << 0;
        const MODIFY_CF = 1 << 1;
        const MODIFY_SF = 1 << 2;
        const MODIFY_ZF = 1 << 3;
        const MODIFY_PF = 1 << 4;
        const MODIFY_OF = 1 << 5;
        const MODIFY_TF = 1 << 6;
        const MODIFY_IF = 1 << 7;
        const MODIFY_DF = 1 << 8;
        const MODIFY_NT = 1 << 9;
        const MODIFY_RF = 1 << 10;
        const PRIOR_OF = 1 << 11;
        const PRIOR_SF = 1 << 12;
        const PRIOR_ZF = 1 << 13;
        const PRIOR_AF = 1 << 14;
        const PRIOR_PF = 1 << 15;
        const PRIOR_CF = 1 << 16;
        const PRIOR_TF = 1 << 17;
        const PRIOR_IF = 1 << 18;
        const PRIOR_DF = 1 << 19;
        const PRIOR_NT = 1 << 20;
        const RESET_OF = 1 << 21;
        const RESET_CF = 1 << 22;
        const RESET_DF = 1 << 23;
        const RESET_IF = 1 << 24;
        const RESET_SF = 1 << 25;
        const RESET_AF = 1 << 26;
        const RESET_TF = 1 << 27;
        const RESET_NT = 1 << 28;
        const RESET_PF = 1 << 29;
        const SET_CF = 1 << 30;
        const SET_DF = 1 << 31;
        const SET_IF = 1 << 32;
        const TEST_OF = 1 << 33;
        const TEST_SF = 1 << 34;
        const TEST_ZF = 1 << 35;
        const TEST_PF = 1 << 36;
        const TEST_CF = 1 << 37;
        const TEST_NT = 1 << 38;
        const TEST_DF = 1 << 39;
        const UNDEFINED_OF = 1 << 40;
        const UNDEFINED_SF = 1 << 41;
        const UNDEFINED_ZF = 1 << 42;
        const UNDEFINED_PF = 1 << 43;
        const UNDEFINED_AF = 1 << 44;
        const UNDEFINED_CF = 1 << 45;
        const RESET_RF = 1 << 46;
        const TEST_RF = 1 << 47;
        const TEST_IF = 1 << 48;
        const TEST_TF = 1 << 49;
        const TEST_AF = 1 << 50;
        const RESET_ZF = 1 << 51;
        const SET_OF = 1 << 52;
        const SET_SF = 1 << 53;
        const SET_ZF = 1 << 54;
        const SET_AF = 1 << 55;
        const SET_PF = 1 << 56;
        const RESET_0F = 1 << 57;
        const RESET_AC = 1 << 58;
    }
}

bitflags::bitflags! {
    pub struct FpuFlags: u64 {
        const MODIFY_C0 = 1 << 0;
        const MODIFY_C1 = 1 << 1;
        const MODIFY_C2 = 1 << 2;
        const MODIFY_C3 = 1 << 3;
        const RESET_C0 = 1 << 4;
        const RESET_C1 = 1 << 5;
        const RESET_C2 = 1 << 6;
        const RESET_C3 = 1 << 7;
        const SET_C0 = 1 << 8;
        const SET_C1 = 1 << 9;
        const SET_C2 = 1 << 10;
        const SET_C3 = 1 << 11;
        const UNDEFINED_C0 = 1 << 12;
        const UNDEFINED_C1 = 1 << 13;
        const UNDEFINED_C2 = 1 << 14;
        const UNDEFINED_C3 = 1 << 15;
        const TEST_C0 = 1 << 16;
        const TEST_C1 = 1 << 17;
        const TEST_C2 = 1 << 18;
        const TEST_C3 = 1 << 19;
    }
}

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

c_enum_big! {
    #[non_exhaustive]
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub enum Reg: u8 {
        @Start = Invalid,
        @End   = Ending,

        Invalid = 0,
        Ah,
        Al,
        Ax,
        Bh,
        Bl,
        Bp,
        Bpl,
        Bx,
        Ch,
        Cl,
        Cs,
        Cx,
        Dh,
        Di,
        Dil,
        Dl,
        Ds,
        Dx,
        Eax,
        Ebp,
        Ebx,
        Ecx,
        Edi,
        Edx,
        Eflags,
        Eip,
        Eiz,
        Es,
        Esi,
        Esp,
        Fpsw,
        Fs,
        Gs,
        Ip,
        Rax,
        Rbp,
        Rbx,
        Rcx,
        Rdi,
        Rdx,
        Rip,
        Riz,
        Rsi,
        Rsp,
        Si,
        Sil,
        Sp,
        Spl,
        Ss,
        Cr0,
        Cr1,
        Cr2,
        Cr3,
        Cr4,
        Cr5,
        Cr6,
        Cr7,
        Cr8,
        Cr9,
        Cr10,
        Cr11,
        Cr12,
        Cr13,
        Cr14,
        Cr15,
        Dr0,
        Dr1,
        Dr2,
        Dr3,
        Dr4,
        Dr5,
        Dr6,
        Dr7,
        Dr8,
        Dr9,
        Dr10,
        Dr11,
        Dr12,
        Dr13,
        Dr14,
        Dr15,
        Fp0,
        Fp1,
        Fp2,
        Fp3,
        Fp4,
        Fp5,
        Fp6,
        Fp7,
        K0,
        K1,
        K2,
        K3,
        K4,
        K5,
        K6,
        K7,
        Mm0,
        Mm1,
        Mm2,
        Mm3,
        Mm4,
        Mm5,
        Mm6,
        Mm7,
        R8,
        R9,
        R10,
        R11,
        R12,
        R13,
        R14,
        R15,
        St0,
        St1,
        St2,
        St3,
        St4,
        St5,
        St6,
        St7,
        Xmm0,
        Xmm1,
        Xmm2,
        Xmm3,
        Xmm4,
        Xmm5,
        Xmm6,
        Xmm7,
        Xmm8,
        Xmm9,
        Xmm10,
        Xmm11,
        Xmm12,
        Xmm13,
        Xmm14,
        Xmm15,
        Xmm16,
        Xmm17,
        Xmm18,
        Xmm19,
        Xmm20,
        Xmm21,
        Xmm22,
        Xmm23,
        Xmm24,
        Xmm25,
        Xmm26,
        Xmm27,
        Xmm28,
        Xmm29,
        Xmm30,
        Xmm31,
        Ymm0,
        Ymm1,
        Ymm2,
        Ymm3,
        Ymm4,
        Ymm5,
        Ymm6,
        Ymm7,
        Ymm8,
        Ymm9,
        Ymm10,
        Ymm11,
        Ymm12,
        Ymm13,
        Ymm14,
        Ymm15,
        Ymm16,
        Ymm17,
        Ymm18,
        Ymm19,
        Ymm20,
        Ymm21,
        Ymm22,
        Ymm23,
        Ymm24,
        Ymm25,
        Ymm26,
        Ymm27,
        Ymm28,
        Ymm29,
        Ymm30,
        Ymm31,
        Zmm0,
        Zmm1,
        Zmm2,
        Zmm3,
        Zmm4,
        Zmm5,
        Zmm6,
        Zmm7,
        Zmm8,
        Zmm9,
        Zmm10,
        Zmm11,
        Zmm12,
        Zmm13,
        Zmm14,
        Zmm15,
        Zmm16,
        Zmm17,
        Zmm18,
        Zmm19,
        Zmm20,
        Zmm21,
        Zmm22,
        Zmm23,
        Zmm24,
        Zmm25,
        Zmm26,
        Zmm27,
        Zmm28,
        Zmm29,
        Zmm30,
        Zmm31,
        R8b,
        R9b,
        R10b,
        R11b,
        R12b,
        R13b,
        R14b,
        R15b,
        R8d,
        R9d,
        R10d,
        R11d,
        R12d,
        R13d,
        R14d,
        R15d,
        R8w,
        R9w,
        R10w,
        R11w,
        R12w,
        R13w,
        R14w,
        R15w,

        #[doc(hidden)]
        Ending,
    }
}

c_enum_big! {
    #[non_exhaustive]
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub enum InsnId: u16 {
        @Start = Invalid,
        @End   = Ending,

        Invalid = 0,

        Aaa,
        Aad,
        Aam,
        Aas,
        Fabs,
        Adc,
        Adcx,
        Add,
        Addpd,
        Addps,
        Addsd,
        Addss,
        Addsubpd,
        Addsubps,
        Fadd,
        Fiadd,
        Faddp,
        Adox,
        Aesdeclast,
        Aesdec,
        Aesenclast,
        Aesenc,
        Aesimc,
        Aeskeygenassist,
        And,
        Andn,
        Andnpd,
        Andnps,
        Andpd,
        Andps,
        Arpl,
        Bextr,
        Blcfill,
        Blci,
        Blcic,
        Blcmsk,
        Blcs,
        Blendpd,
        Blendps,
        Blendvpd,
        Blendvps,
        Blsfill,
        Blsi,
        Blsic,
        Blsmsk,
        Blsr,
        Bound,
        Bsf,
        Bsr,
        Bswap,
        Bt,
        Btc,
        Btr,
        Bts,
        Bzhi,
        Call,
        Cbw,
        Cdq,
        Cdqe,
        Fchs,
        Clac,
        Clc,
        Cld,
        Clflush,
        Clflushopt,
        Clgi,
        Cli,
        Clts,
        Clwb,
        Cmc,
        Cmova,
        Cmovae,
        Cmovb,
        Cmovbe,
        Fcmovbe,
        Fcmovb,
        Cmove,
        Fcmove,
        Cmovg,
        Cmovge,
        Cmovl,
        Cmovle,
        Fcmovnbe,
        Fcmovnb,
        Cmovne,
        Fcmovne,
        Cmovno,
        Cmovnp,
        Fcmovnu,
        Cmovns,
        Cmovo,
        Cmovp,
        Fcmovu,
        Cmovs,
        Cmp,
        Cmpsb,
        Cmpsq,
        Cmpsw,
        Cmpxchg16b,
        Cmpxchg,
        Cmpxchg8b,
        Comisd,
        Comiss,
        Fcomp,
        Fcomip,
        Fcomi,
        Fcom,
        Fcos,
        Cpuid,
        Cqo,
        Crc32,
        Cvtdq2pd,
        Cvtdq2ps,
        Cvtpd2dq,
        Cvtpd2ps,
        Cvtps2dq,
        Cvtps2pd,
        Cvtsd2si,
        Cvtsd2ss,
        Cvtsi2sd,
        Cvtsi2ss,
        Cvtss2sd,
        Cvtss2si,
        Cvttpd2dq,
        Cvttps2dq,
        Cvttsd2si,
        Cvttss2si,
        Cwd,
        Cwde,
        Daa,
        Das,
        Data16,
        Dec,
        Div,
        Divpd,
        Divps,
        Fdivr,
        Fidivr,
        Fdivrp,
        Divsd,
        Divss,
        Fdiv,
        Fidiv,
        Fdivp,
        Dppd,
        Dpps,
        Ret,
        Encls,
        Enclu,
        Enter,
        Extractps,
        Extrq,
        F2xm1,
        Lcall,
        Ljmp,
        Fbld,
        Fbstp,
        Fcompp,
        Fdecstp,
        Femms,
        Ffree,
        Ficom,
        Ficomp,
        Fincstp,
        Fldcw,
        Fldenv,
        Fldl2e,
        Fldl2t,
        Fldlg2,
        Fldln2,
        Fldpi,
        Fnclex,
        Fninit,
        Fnop,
        Fnstcw,
        Fnstsw,
        Fpatan,
        Fprem,
        Fprem1,
        Fptan,
        Ffreep,
        Frndint,
        Frstor,
        Fnsave,
        Fscale,
        Fsetpm,
        Fsincos,
        Fnstenv,
        Fxam,
        Fxrstor,
        Fxrstor64,
        Fxsave,
        Fxsave64,
        Fxtract,
        Fyl2x,
        Fyl2xp1,
        Movapd,
        Movaps,
        Orpd,
        Orps,
        Vmovapd,
        Vmovaps,
        Xorpd,
        Xorps,
        Getsec,
        Haddpd,
        Haddps,
        Hlt,
        Hsubpd,
        Hsubps,
        Idiv,
        Fild,
        Imul,
        In,
        Inc,
        Insb,
        Insertps,
        Insertq,
        Insd,
        Insw,
        Int,
        Int1,
        Int3,
        Into,
        Invd,
        Invept,
        Invlpg,
        Invlpga,
        Invpcid,
        Invvpid,
        Iret,
        Iretd,
        Iretq,
        Fisttp,
        Fist,
        Fistp,
        Ucomisd,
        Ucomiss,
        Vcomisd,
        Vcomiss,
        Vcvtsd2ss,
        Vcvtsi2sd,
        Vcvtsi2ss,
        Vcvtss2sd,
        Vcvttsd2si,
        Vcvttsd2usi,
        Vcvttss2si,
        Vcvttss2usi,
        Vcvtusi2sd,
        Vcvtusi2ss,
        Vucomisd,
        Vucomiss,
        Jae,
        Ja,
        Jbe,
        Jb,
        Jcxz,
        Jecxz,
        Je,
        Jge,
        Jg,
        Jle,
        Jl,
        Jmp,
        Jne,
        Jno,
        Jnp,
        Jns,
        Jo,
        Jp,
        Jrcxz,
        Js,
        Kandb,
        Kandd,
        Kandnb,
        Kandnd,
        Kandnq,
        Kandnw,
        Kandq,
        Kandw,
        Kmovb,
        Kmovd,
        Kmovq,
        Kmovw,
        Knotb,
        Knotd,
        Knotq,
        Knotw,
        Korb,
        Kord,
        Korq,
        Kortestb,
        Kortestd,
        Kortestq,
        Kortestw,
        Korw,
        Kshiftlb,
        Kshiftld,
        Kshiftlq,
        Kshiftlw,
        Kshiftrb,
        Kshiftrd,
        Kshiftrq,
        Kshiftrw,
        Kunpckbw,
        Kxnorb,
        Kxnord,
        Kxnorq,
        Kxnorw,
        Kxorb,
        Kxord,
        Kxorq,
        Kxorw,
        Lahf,
        Lar,
        Lddqu,
        Ldmxcsr,
        Lds,
        Fldz,
        Fld1,
        Fld,
        Lea,
        Leave,
        Les,
        Lfence,
        Lfs,
        Lgdt,
        Lgs,
        Lidt,
        Lldt,
        Lmsw,
        Or,
        Sub,
        Xor,
        Lodsb,
        Lodsd,
        Lodsq,
        Lodsw,
        Loop,
        Loope,
        Loopne,
        Retf,
        Retfq,
        Lsl,
        Lss,
        Ltr,
        Xadd,
        Lzcnt,
        Maskmovdqu,
        Maxpd,
        Maxps,
        Maxsd,
        Maxss,
        Mfence,
        Minpd,
        Minps,
        Minsd,
        Minss,
        Cvtpd2pi,
        Cvtpi2pd,
        Cvtpi2ps,
        Cvtps2pi,
        Cvttpd2pi,
        Cvttps2pi,
        Emms,
        Maskmovq,
        Movd,
        Movdq2q,
        Movntq,
        Movq2dq,
        Movq,
        Pabsb,
        Pabsd,
        Pabsw,
        Packssdw,
        Packsswb,
        Packuswb,
        Paddb,
        Paddd,
        Paddq,
        Paddsb,
        Paddsw,
        Paddusb,
        Paddusw,
        Paddw,
        Palignr,
        Pandn,
        Pand,
        Pavgb,
        Pavgw,
        Pcmpeqb,
        Pcmpeqd,
        Pcmpeqw,
        Pcmpgtb,
        Pcmpgtd,
        Pcmpgtw,
        Pextrw,
        Phaddsw,
        Phaddw,
        Phaddd,
        Phsubd,
        Phsubsw,
        Phsubw,
        Pinsrw,
        Pmaddubsw,
        Pmaddwd,
        Pmaxsw,
        Pmaxub,
        Pminsw,
        Pminub,
        Pmovmskb,
        Pmulhrsw,
        Pmulhuw,
        Pmulhw,
        Pmullw,
        Pmuludq,
        Por,
        Psadbw,
        Pshufb,
        Pshufw,
        Psignb,
        Psignd,
        Psignw,
        Pslld,
        Psllq,
        Psllw,
        Psrad,
        Psraw,
        Psrld,
        Psrlq,
        Psrlw,
        Psubb,
        Psubd,
        Psubq,
        Psubsb,
        Psubsw,
        Psubusb,
        Psubusw,
        Psubw,
        Punpckhbw,
        Punpckhdq,
        Punpckhwd,
        Punpcklbw,
        Punpckldq,
        Punpcklwd,
        Pxor,
        Monitor,
        Montmul,
        Mov,
        Movabs,
        Movbe,
        Movddup,
        Movdqa,
        Movdqu,
        Movhlps,
        Movhpd,
        Movhps,
        Movlhps,
        Movlpd,
        Movlps,
        Movmskpd,
        Movmskps,
        Movntdqa,
        Movntdq,
        Movnti,
        Movntpd,
        Movntps,
        Movntsd,
        Movntss,
        Movsb,
        Movsd,
        Movshdup,
        Movsldup,
        Movsq,
        Movss,
        Movsw,
        Movsx,
        Movsxd,
        Movupd,
        Movups,
        Movzx,
        Mpsadbw,
        Mul,
        Mulpd,
        Mulps,
        Mulsd,
        Mulss,
        Mulx,
        Fmul,
        Fimul,
        Fmulp,
        Mwait,
        Neg,
        Nop,
        Not,
        Out,
        Outsb,
        Outsd,
        Outsw,
        Packusdw,
        Pause,
        Pavgusb,
        Pblendvb,
        Pblendw,
        Pclmulqdq,
        Pcmpeqq,
        Pcmpestri,
        Pcmpestrm,
        Pcmpgtq,
        Pcmpistri,
        Pcmpistrm,
        Pcommit,
        Pdep,
        Pext,
        Pextrb,
        Pextrd,
        Pextrq,
        Pf2id,
        Pf2iw,
        Pfacc,
        Pfadd,
        Pfcmpeq,
        Pfcmpge,
        Pfcmpgt,
        Pfmax,
        Pfmin,
        Pfmul,
        Pfnacc,
        Pfpnacc,
        Pfrcpit1,
        Pfrcpit2,
        Pfrcp,
        Pfrsqit1,
        Pfrsqrt,
        Pfsubr,
        Pfsub,
        Phminposuw,
        Pi2fd,
        Pi2fw,
        Pinsrb,
        Pinsrd,
        Pinsrq,
        Pmaxsb,
        Pmaxsd,
        Pmaxud,
        Pmaxuw,
        Pminsb,
        Pminsd,
        Pminud,
        Pminuw,
        Pmovsxbd,
        Pmovsxbq,
        Pmovsxbw,
        Pmovsxdq,
        Pmovsxwd,
        Pmovsxwq,
        Pmovzxbd,
        Pmovzxbq,
        Pmovzxbw,
        Pmovzxdq,
        Pmovzxwd,
        Pmovzxwq,
        Pmuldq,
        Pmulhrw,
        Pmulld,
        Pop,
        Popaw,
        Popal,
        Popcnt,
        Popf,
        Popfd,
        Popfq,
        Prefetch,
        Prefetchnta,
        Prefetcht0,
        Prefetcht1,
        Prefetcht2,
        Prefetchw,
        Pshufd,
        Pshufhw,
        Pshuflw,
        Pslldq,
        Psrldq,
        Pswapd,
        Ptest,
        Punpckhqdq,
        Punpcklqdq,
        Push,
        Pushaw,
        Pushal,
        Pushf,
        Pushfd,
        Pushfq,
        Rcl,
        Rcpps,
        Rcpss,
        Rcr,
        Rdfsbase,
        Rdgsbase,
        Rdmsr,
        Rdpmc,
        Rdrand,
        Rdseed,
        Rdtsc,
        Rdtscp,
        Rol,
        Ror,
        Rorx,
        Roundpd,
        Roundps,
        Roundsd,
        Roundss,
        Rsm,
        Rsqrtps,
        Rsqrtss,
        Sahf,
        Sal,
        Salc,
        Sar,
        Sarx,
        Sbb,
        Scasb,
        Scasd,
        Scasq,
        Scasw,
        Setae,
        Seta,
        Setbe,
        Setb,
        Sete,
        Setge,
        Setg,
        Setle,
        Setl,
        Setne,
        Setno,
        Setnp,
        Setns,
        Seto,
        Setp,
        Sets,
        Sfence,
        Sgdt,
        Sha1msg1,
        Sha1msg2,
        Sha1nexte,
        Sha1rnds4,
        Sha256msg1,
        Sha256msg2,
        Sha256rnds2,
        Shl,
        Shld,
        Shlx,
        Shr,
        Shrd,
        Shrx,
        Shufpd,
        Shufps,
        Sidt,
        Fsin,
        Skinit,
        Sldt,
        Smsw,
        Sqrtpd,
        Sqrtps,
        Sqrtsd,
        Sqrtss,
        Fsqrt,
        Stac,
        Stc,
        Std,
        Stgi,
        Sti,
        Stmxcsr,
        Stosb,
        Stosd,
        Stosq,
        Stosw,
        Str,
        Fst,
        Fstp,
        Fstpnce,
        Fxch,
        Subpd,
        Subps,
        Fsubr,
        Fisubr,
        Fsubrp,
        Subsd,
        Subss,
        Fsub,
        Fisub,
        Fsubp,
        Swapgs,
        Syscall,
        Sysenter,
        Sysexit,
        Sysret,
        T1mskc,
        Test,
        Ud2,
        Ftst,
        Tzcnt,
        Tzmsk,
        Fucomip,
        Fucomi,
        Fucompp,
        Fucomp,
        Fucom,
        Ud2b,
        Unpckhpd,
        Unpckhps,
        Unpcklpd,
        Unpcklps,
        Vaddpd,
        Vaddps,
        Vaddsd,
        Vaddss,
        Vaddsubpd,
        Vaddsubps,
        Vaesdeclast,
        Vaesdec,
        Vaesenclast,
        Vaesenc,
        Vaesimc,
        Vaeskeygenassist,
        Valignd,
        Valignq,
        Vandnpd,
        Vandnps,
        Vandpd,
        Vandps,
        Vblendmpd,
        Vblendmps,
        Vblendpd,
        Vblendps,
        Vblendvpd,
        Vblendvps,
        Vbroadcastf128,
        Vbroadcasti32x4,
        Vbroadcasti64x4,
        Vbroadcastsd,
        Vbroadcastss,
        Vcompresspd,
        Vcompressps,
        Vcvtdq2pd,
        Vcvtdq2ps,
        Vcvtpd2dqx,
        Vcvtpd2dq,
        Vcvtpd2psx,
        Vcvtpd2ps,
        Vcvtpd2udq,
        Vcvtph2ps,
        Vcvtps2dq,
        Vcvtps2pd,
        Vcvtps2ph,
        Vcvtps2udq,
        Vcvtsd2si,
        Vcvtsd2usi,
        Vcvtss2si,
        Vcvtss2usi,
        Vcvttpd2dqx,
        Vcvttpd2dq,
        Vcvttpd2udq,
        Vcvttps2dq,
        Vcvttps2udq,
        Vcvtudq2pd,
        Vcvtudq2ps,
        Vdivpd,
        Vdivps,
        Vdivsd,
        Vdivss,
        Vdppd,
        Vdpps,
        Verr,
        Verw,
        Vexp2pd,
        Vexp2ps,
        Vexpandpd,
        Vexpandps,
        Vextractf128,
        Vextractf32x4,
        Vextractf64x4,
        Vextracti128,
        Vextracti32x4,
        Vextracti64x4,
        Vextractps,
        Vfmadd132pd,
        Vfmadd132ps,
        Vfmaddpd,
        Vfmadd213pd,
        Vfmadd231pd,
        Vfmaddps,
        Vfmadd213ps,
        Vfmadd231ps,
        Vfmaddsd,
        Vfmadd213sd,
        Vfmadd132sd,
        Vfmadd231sd,
        Vfmaddss,
        Vfmadd213ss,
        Vfmadd132ss,
        Vfmadd231ss,
        Vfmaddsub132pd,
        Vfmaddsub132ps,
        Vfmaddsubpd,
        Vfmaddsub213pd,
        Vfmaddsub231pd,
        Vfmaddsubps,
        Vfmaddsub213ps,
        Vfmaddsub231ps,
        Vfmsub132pd,
        Vfmsub132ps,
        Vfmsubadd132pd,
        Vfmsubadd132ps,
        Vfmsubaddpd,
        Vfmsubadd213pd,
        Vfmsubadd231pd,
        Vfmsubaddps,
        Vfmsubadd213ps,
        Vfmsubadd231ps,
        Vfmsubpd,
        Vfmsub213pd,
        Vfmsub231pd,
        Vfmsubps,
        Vfmsub213ps,
        Vfmsub231ps,
        Vfmsubsd,
        Vfmsub213sd,
        Vfmsub132sd,
        Vfmsub231sd,
        Vfmsubss,
        Vfmsub213ss,
        Vfmsub132ss,
        Vfmsub231ss,
        Vfnmadd132pd,
        Vfnmadd132ps,
        Vfnmaddpd,
        Vfnmadd213pd,
        Vfnmadd231pd,
        Vfnmaddps,
        Vfnmadd213ps,
        Vfnmadd231ps,
        Vfnmaddsd,
        Vfnmadd213sd,
        Vfnmadd132sd,
        Vfnmadd231sd,
        Vfnmaddss,
        Vfnmadd213ss,
        Vfnmadd132ss,
        Vfnmadd231ss,
        Vfnmsub132pd,
        Vfnmsub132ps,
        Vfnmsubpd,
        Vfnmsub213pd,
        Vfnmsub231pd,
        Vfnmsubps,
        Vfnmsub213ps,
        Vfnmsub231ps,
        Vfnmsubsd,
        Vfnmsub213sd,
        Vfnmsub132sd,
        Vfnmsub231sd,
        Vfnmsubss,
        Vfnmsub213ss,
        Vfnmsub132ss,
        Vfnmsub231ss,
        Vfrczpd,
        Vfrczps,
        Vfrczsd,
        Vfrczss,
        Vorpd,
        Vorps,
        Vxorpd,
        Vxorps,
        Vgatherdpd,
        Vgatherdps,
        Vgatherpf0dpd,
        Vgatherpf0dps,
        Vgatherpf0qpd,
        Vgatherpf0qps,
        Vgatherpf1dpd,
        Vgatherpf1dps,
        Vgatherpf1qpd,
        Vgatherpf1qps,
        Vgatherqpd,
        Vgatherqps,
        Vhaddpd,
        Vhaddps,
        Vhsubpd,
        Vhsubps,
        Vinsertf128,
        Vinsertf32x4,
        Vinsertf32x8,
        Vinsertf64x2,
        Vinsertf64x4,
        Vinserti128,
        Vinserti32x4,
        Vinserti32x8,
        Vinserti64x2,
        Vinserti64x4,
        Vinsertps,
        Vlddqu,
        Vldmxcsr,
        Vmaskmovdqu,
        Vmaskmovpd,
        Vmaskmovps,
        Vmaxpd,
        Vmaxps,
        Vmaxsd,
        Vmaxss,
        Vmcall,
        Vmclear,
        Vmfunc,
        Vminpd,
        Vminps,
        Vminsd,
        Vminss,
        Vmlaunch,
        Vmload,
        Vmmcall,
        Vmovq,
        Vmovddup,
        Vmovd,
        Vmovdqa32,
        Vmovdqa64,
        Vmovdqa,
        Vmovdqu16,
        Vmovdqu32,
        Vmovdqu64,
        Vmovdqu8,
        Vmovdqu,
        Vmovhlps,
        Vmovhpd,
        Vmovhps,
        Vmovlhps,
        Vmovlpd,
        Vmovlps,
        Vmovmskpd,
        Vmovmskps,
        Vmovntdqa,
        Vmovntdq,
        Vmovntpd,
        Vmovntps,
        Vmovsd,
        Vmovshdup,
        Vmovsldup,
        Vmovss,
        Vmovupd,
        Vmovups,
        Vmpsadbw,
        Vmptrld,
        Vmptrst,
        Vmread,
        Vmresume,
        Vmrun,
        Vmsave,
        Vmulpd,
        Vmulps,
        Vmulsd,
        Vmulss,
        Vmwrite,
        Vmxoff,
        Vmxon,
        Vpabsb,
        Vpabsd,
        Vpabsq,
        Vpabsw,
        Vpackssdw,
        Vpacksswb,
        Vpackusdw,
        Vpackuswb,
        Vpaddb,
        Vpaddd,
        Vpaddq,
        Vpaddsb,
        Vpaddsw,
        Vpaddusb,
        Vpaddusw,
        Vpaddw,
        Vpalignr,
        Vpandd,
        Vpandnd,
        Vpandnq,
        Vpandn,
        Vpandq,
        Vpand,
        Vpavgb,
        Vpavgw,
        Vpblendd,
        Vpblendmb,
        Vpblendmd,
        Vpblendmq,
        Vpblendmw,
        Vpblendvb,
        Vpblendw,
        Vpbroadcastb,
        Vpbroadcastd,
        Vpbroadcastmb2q,
        Vpbroadcastmw2d,
        Vpbroadcastq,
        Vpbroadcastw,
        Vpclmulqdq,
        Vpcmov,
        Vpcmpb,
        Vpcmpd,
        Vpcmpeqb,
        Vpcmpeqd,
        Vpcmpeqq,
        Vpcmpeqw,
        Vpcmpestri,
        Vpcmpestrm,
        Vpcmpgtb,
        Vpcmpgtd,
        Vpcmpgtq,
        Vpcmpgtw,
        Vpcmpistri,
        Vpcmpistrm,
        Vpcmpq,
        Vpcmpub,
        Vpcmpud,
        Vpcmpuq,
        Vpcmpuw,
        Vpcmpw,
        Vpcomb,
        Vpcomd,
        Vpcompressd,
        Vpcompressq,
        Vpcomq,
        Vpcomub,
        Vpcomud,
        Vpcomuq,
        Vpcomuw,
        Vpcomw,
        Vpconflictd,
        Vpconflictq,
        Vperm2f128,
        Vperm2i128,
        Vpermd,
        Vpermi2d,
        Vpermi2pd,
        Vpermi2ps,
        Vpermi2q,
        Vpermil2pd,
        Vpermil2ps,
        Vpermilpd,
        Vpermilps,
        Vpermpd,
        Vpermps,
        Vpermq,
        Vpermt2d,
        Vpermt2pd,
        Vpermt2ps,
        Vpermt2q,
        Vpexpandd,
        Vpexpandq,
        Vpextrb,
        Vpextrd,
        Vpextrq,
        Vpextrw,
        Vpgatherdd,
        Vpgatherdq,
        Vpgatherqd,
        Vpgatherqq,
        Vphaddbd,
        Vphaddbq,
        Vphaddbw,
        Vphadddq,
        Vphaddd,
        Vphaddsw,
        Vphaddubd,
        Vphaddubq,
        Vphaddubw,
        Vphaddudq,
        Vphadduwd,
        Vphadduwq,
        Vphaddwd,
        Vphaddwq,
        Vphaddw,
        Vphminposuw,
        Vphsubbw,
        Vphsubdq,
        Vphsubd,
        Vphsubsw,
        Vphsubwd,
        Vphsubw,
        Vpinsrb,
        Vpinsrd,
        Vpinsrq,
        Vpinsrw,
        Vplzcntd,
        Vplzcntq,
        Vpmacsdd,
        Vpmacsdqh,
        Vpmacsdql,
        Vpmacssdd,
        Vpmacssdqh,
        Vpmacssdql,
        Vpmacsswd,
        Vpmacssww,
        Vpmacswd,
        Vpmacsww,
        Vpmadcsswd,
        Vpmadcswd,
        Vpmaddubsw,
        Vpmaddwd,
        Vpmaskmovd,
        Vpmaskmovq,
        Vpmaxsb,
        Vpmaxsd,
        Vpmaxsq,
        Vpmaxsw,
        Vpmaxub,
        Vpmaxud,
        Vpmaxuq,
        Vpmaxuw,
        Vpminsb,
        Vpminsd,
        Vpminsq,
        Vpminsw,
        Vpminub,
        Vpminud,
        Vpminuq,
        Vpminuw,
        Vpmovdb,
        Vpmovdw,
        Vpmovm2b,
        Vpmovm2d,
        Vpmovm2q,
        Vpmovm2w,
        Vpmovmskb,
        Vpmovqb,
        Vpmovqd,
        Vpmovqw,
        Vpmovsdb,
        Vpmovsdw,
        Vpmovsqb,
        Vpmovsqd,
        Vpmovsqw,
        Vpmovsxbd,
        Vpmovsxbq,
        Vpmovsxbw,
        Vpmovsxdq,
        Vpmovsxwd,
        Vpmovsxwq,
        Vpmovusdb,
        Vpmovusdw,
        Vpmovusqb,
        Vpmovusqd,
        Vpmovusqw,
        Vpmovzxbd,
        Vpmovzxbq,
        Vpmovzxbw,
        Vpmovzxdq,
        Vpmovzxwd,
        Vpmovzxwq,
        Vpmuldq,
        Vpmulhrsw,
        Vpmulhuw,
        Vpmulhw,
        Vpmulld,
        Vpmullq,
        Vpmullw,
        Vpmuludq,
        Vpord,
        Vporq,
        Vpor,
        Vpperm,
        Vprotb,
        Vprotd,
        Vprotq,
        Vprotw,
        Vpsadbw,
        Vpscatterdd,
        Vpscatterdq,
        Vpscatterqd,
        Vpscatterqq,
        Vpshab,
        Vpshad,
        Vpshaq,
        Vpshaw,
        Vpshlb,
        Vpshld,
        Vpshlq,
        Vpshlw,
        Vpshufb,
        Vpshufd,
        Vpshufhw,
        Vpshuflw,
        Vpsignb,
        Vpsignd,
        Vpsignw,
        Vpslldq,
        Vpslld,
        Vpsllq,
        Vpsllvd,
        Vpsllvq,
        Vpsllw,
        Vpsrad,
        Vpsraq,
        Vpsravd,
        Vpsravq,
        Vpsraw,
        Vpsrldq,
        Vpsrld,
        Vpsrlq,
        Vpsrlvd,
        Vpsrlvq,
        Vpsrlw,
        Vpsubb,
        Vpsubd,
        Vpsubq,
        Vpsubsb,
        Vpsubsw,
        Vpsubusb,
        Vpsubusw,
        Vpsubw,
        Vptestmd,
        Vptestmq,
        Vptestnmd,
        Vptestnmq,
        Vptest,
        Vpunpckhbw,
        Vpunpckhdq,
        Vpunpckhqdq,
        Vpunpckhwd,
        Vpunpcklbw,
        Vpunpckldq,
        Vpunpcklqdq,
        Vpunpcklwd,
        Vpxord,
        Vpxorq,
        Vpxor,
        Vrcp14pd,
        Vrcp14ps,
        Vrcp14sd,
        Vrcp14ss,
        Vrcp28pd,
        Vrcp28ps,
        Vrcp28sd,
        Vrcp28ss,
        Vrcpps,
        Vrcpss,
        Vrndscalepd,
        Vrndscaleps,
        Vrndscalesd,
        Vrndscaless,
        Vroundpd,
        Vroundps,
        Vroundsd,
        Vroundss,
        Vrsqrt14pd,
        Vrsqrt14ps,
        Vrsqrt14sd,
        Vrsqrt14ss,
        Vrsqrt28pd,
        Vrsqrt28ps,
        Vrsqrt28sd,
        Vrsqrt28ss,
        Vrsqrtps,
        Vrsqrtss,
        Vscatterdpd,
        Vscatterdps,
        Vscatterpf0dpd,
        Vscatterpf0dps,
        Vscatterpf0qpd,
        Vscatterpf0qps,
        Vscatterpf1dpd,
        Vscatterpf1dps,
        Vscatterpf1qpd,
        Vscatterpf1qps,
        Vscatterqpd,
        Vscatterqps,
        Vshufpd,
        Vshufps,
        Vsqrtpd,
        Vsqrtps,
        Vsqrtsd,
        Vsqrtss,
        Vstmxcsr,
        Vsubpd,
        Vsubps,
        Vsubsd,
        Vsubss,
        Vtestpd,
        Vtestps,
        Vunpckhpd,
        Vunpckhps,
        Vunpcklpd,
        Vunpcklps,
        Vzeroall,
        Vzeroupper,
        Wait,
        Wbinvd,
        Wrfsbase,
        Wrgsbase,
        Wrmsr,
        Xabort,
        Xacquire,
        Xbegin,
        Xchg,
        Xcryptcbc,
        Xcryptcfb,
        Xcryptctr,
        Xcryptecb,
        Xcryptofb,
        Xend,
        Xgetbv,
        Xlatb,
        Xrelease,
        Xrstor,
        Xrstor64,
        Xrstors,
        Xrstors64,
        Xsave,
        Xsave64,
        Xsavec,
        Xsavec64,
        Xsaveopt,
        Xsaveopt64,
        Xsaves,
        Xsaves64,
        Xsetbv,
        Xsha1,
        Xsha256,
        Xstore,
        Xtest,
        Fdisi8087Nop,
        Feni8087Nop,

        // pseudo instructions
        Cmpss,
        Cmpeqss,
        Cmpltss,
        Cmpless,
        Cmpunordss,
        Cmpneqss,
        Cmpnltss,
        Cmpnless,
        Cmpordss,

        Cmpsd,
        Cmpeqsd,
        Cmpltsd,
        Cmplesd,
        Cmpunordsd,
        Cmpneqsd,
        Cmpnltsd,
        Cmpnlesd,
        Cmpordsd,

        Cmpps,
        Cmpeqps,
        Cmpltps,
        Cmpleps,
        Cmpunordps,
        Cmpneqps,
        Cmpnltps,
        Cmpnleps,
        Cmpordps,

        Cmppd,
        Cmpeqpd,
        Cmpltpd,
        Cmplepd,
        Cmpunordpd,
        Cmpneqpd,
        Cmpnltpd,
        Cmpnlepd,
        Cmpordpd,

        Vcmpss,
        Vcmpeqss,
        Vcmpltss,
        Vcmpless,
        Vcmpunordss,
        Vcmpneqss,
        Vcmpnltss,
        Vcmpnless,
        Vcmpordss,
        VcmpeqUqss,
        Vcmpngess,
        Vcmpngtss,
        Vcmpfalsess,
        VcmpneqOqss,
        Vcmpgess,
        Vcmpgtss,
        Vcmptruess,
        VcmpeqOsss,
        VcmpltOqss,
        VcmpleOqss,
        VcmpunordSss,
        VcmpneqUsss,
        VcmpnltUqss,
        VcmpnleUqss,
        VcmpordSss,
        VcmpeqUsss,
        VcmpngeUqss,
        VcmpngtUqss,
        VcmpfalseOsss,
        VcmpneqOsss,
        VcmpgeOqss,
        VcmpgtOqss,
        VcmptrueUsss,

        Vcmpsd,
        Vcmpeqsd,
        Vcmpltsd,
        Vcmplesd,
        Vcmpunordsd,
        Vcmpneqsd,
        Vcmpnltsd,
        Vcmpnlesd,
        Vcmpordsd,
        VcmpeqUqsd,
        Vcmpngesd,
        Vcmpngtsd,
        Vcmpfalsesd,
        VcmpneqOqsd,
        Vcmpgesd,
        Vcmpgtsd,
        Vcmptruesd,
        VcmpeqOssd,
        VcmpltOqsd,
        VcmpleOqsd,
        VcmpunordSsd,
        VcmpneqUssd,
        VcmpnltUqsd,
        VcmpnleUqsd,
        VcmpordSsd,
        VcmpeqUssd,
        VcmpngeUqsd,
        VcmpngtUqsd,
        VcmpfalseOssd,
        VcmpneqOssd,
        VcmpgeOqsd,
        VcmpgtOqsd,
        VcmptrueUssd,

        Vcmpps,
        Vcmpeqps,
        Vcmpltps,
        Vcmpleps,
        Vcmpunordps,
        Vcmpneqps,
        Vcmpnltps,
        Vcmpnleps,
        Vcmpordps,
        VcmpeqUqps,
        Vcmpngeps,
        Vcmpngtps,
        Vcmpfalseps,
        VcmpneqOqps,
        Vcmpgeps,
        Vcmpgtps,
        Vcmptrueps,
        VcmpeqOsps,
        VcmpltOqps,
        VcmpleOqps,
        VcmpunordSps,
        VcmpneqUsps,
        VcmpnltUqps,
        VcmpnleUqps,
        VcmpordSps,
        VcmpeqUsps,
        VcmpngeUqps,
        VcmpngtUqps,
        VcmpfalseOsps,
        VcmpneqOsps,
        VcmpgeOqps,
        VcmpgtOqps,
        VcmptrueUsps,

        Vcmppd,
        Vcmpeqpd,
        Vcmpltpd,
        Vcmplepd,
        Vcmpunordpd,
        Vcmpneqpd,
        Vcmpnltpd,
        Vcmpnlepd,
        Vcmpordpd,
        VcmpeqUqpd,
        Vcmpngepd,
        Vcmpngtpd,
        Vcmpfalsepd,
        VcmpneqOqpd,
        Vcmpgepd,
        Vcmpgtpd,
        Vcmptruepd,
        VcmpeqOspd,
        VcmpltOqpd,
        VcmpleOqpd,
        VcmpunordSpd,
        VcmpneqUspd,
        VcmpnltUqpd,
        VcmpnleUqpd,
        VcmpordSpd,
        VcmpeqUspd,
        VcmpngeUqpd,
        VcmpngtUqpd,
        VcmpfalseOspd,
        VcmpneqOspd,
        VcmpgeOqpd,
        VcmpgtOqpd,
        VcmptrueUspd,

        Ud0,
        Endbr32,
        Endbr64,

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
        /// All interrupt instructions (int+syscall)
        Int,
        /// All interrupt return instructions
        Iret,
        /// All privileged instructions
        Privilege,
        /// All relative branching instructions
        BranchRelative,

        // Architecture-specific groups
        /// All virtualization instructions (VT-x + AMD-V)
        VM = 128,
        _3dnow,
        Aes,
        Adx,
        Avx,
        Avx2,
        Avx512,
        Bmi,
        Bmi2,
        Cmov,
        F16c,
        Fma,
        Fma4,
        Fsgsbase,
        Hle,
        Mmx,
        Mode32,
        Mode64,
        Rtm,
        Sha,
        Sse1,
        Sse2,
        Sse3,
        Sse41,
        Sse42,
        Sse4a,
        Ssse3,
        Pclmul,
        Xop,
        Cdi,
        Eri,
        Tbm,
        _16bitmode,
        Not64bitmode,
        Sgx,
        Dqi,
        Bwi,
        Pfi,
        Vlx,
        Smap,
        Novlx,
        Fpu,

        Ending,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn x86_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_x86)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
            sys::get_test_val("alignof(cs_x86)")
        );
    }

    #[test]
    fn x86_enum_size() {
        assert_eq!(Reg::Ending.to_c(), sys::get_test_val("X86_REG_ENDING") as _);
        assert_eq!(
            InsnId::Ending.to_c(),
            sys::get_test_val("X86_INS_ENDING") as _
        );
        assert_eq!(
            InsnGroup::Ending.to_c(),
            sys::get_test_val("X86_GRP_ENDING") as _
        );
    }
}
