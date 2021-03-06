use crate::arch::{
    arm, arm64, evm, m680x, m68k, mips, mos65xx, ppc, sparc, sysz, tms320c64x, x86, xcore,
    InsnGroup, Reg,
};
use crate::{sys, util, Arch};
use core::marker::PhantomData;

const MNEMONIC_SIZE: usize = 32;

/// Information about a disassembled instruction.
#[repr(C)]
pub struct Insn<'a> {
    /// Instruction ID (basically a numeric ID for the instruction mnemonic)
    /// Find the instruction id in the '\[ARCH\]_insn' enum in the header file
    /// of corresponding architecture, such as 'arm_insn' in arm.h for ARM,
    /// 'x86_insn' in x86.h for X86, etc...
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    /// NOTE: in Skipdata mode, "data" instruction has 0 for this id field.
    pub(crate) id: libc::c_uint,

    /// Address (EIP) of this instruction
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    address: u64,

    /// Size of this instruction
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    size: u16,

    /// Machine bytes of this instruction, with number of bytes indicated by @size above
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    bytes: [u8; 24],

    /// Ascii text of instruction mnemonic
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    mnemonic: [libc::c_char; MNEMONIC_SIZE],

    /// Ascii text of instruction operands
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    op_str: [libc::c_char; 160],

    /// Pointer to cs_detail.
    /// NOTE: detail pointer is only valid when both requirements below are met:
    /// (1) CS_OP_DETAIL = CS_OPT_ON
    /// (2) Engine is not in Skipdata mode (CS_OP_SKIPDATA option set to CS_OPT_ON)
    ///
    /// NOTE 2: when in Skipdata mode, or when detail mode is OFF, even if this pointer
    ///     is not NULL, its content is still irrelevant.
    pub(crate) detail: *mut DetailsInner,

    /// Phantom data to tie the lifetime of the Insn to the Capstone instance.
    _phan: PhantomData<&'a ()>,
}

impl<'a> Insn<'a> {
    /// Returns trhe address of this instruction.
    #[inline]
    pub fn address(&self) -> u64 {
        self.address
    }

    /// Returns the size of this instruction in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.size as usize
    }

    /// Returns the machine bytes of this instruction.
    /// The returned slice will have the same size as the return
    /// value of [`Insn::size`]
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.bytes.as_ptr(), self.size()) }
    }

    /// Returns the instruction mnemonic.
    #[inline]
    pub fn mnemonic(&self) -> &str {
        unsafe { util::cstr(self.mnemonic.as_ptr(), MNEMONIC_SIZE) }
    }

    /// Returns the instruction operands as a string.
    #[inline]
    pub fn operands(&self) -> &str {
        unsafe { util::cstr(self.op_str.as_ptr(), 160) }
    }
}

/// A buffer of disassembled instructions.
pub struct InsnBuffer<'a> {
    inner: *mut Insn<'a>,
    count: usize,
    _phan: PhantomData<&'a Insn<'a>>,
}

impl<'a> InsnBuffer<'a> {
    pub(crate) fn new(insn: *mut Insn<'a>, count: usize) -> InsnBuffer<'a> {
        InsnBuffer {
            inner: insn,
            count,
            _phan: PhantomData,
        }
    }

    /// Frees the `Insn`(`cs_insn`) if it is not currently null
    /// then clears the pointer and count.
    fn free(&mut self) {
        if self.count == 0 || self.inner.is_null() {
            return;
        }
        unsafe { sys::cs_free(self.inner as *mut Insn, self.count as libc::size_t) };
        self.inner = core::ptr::null_mut();
        self.count = 0;
    }
}

impl<'a> core::ops::Deref for InsnBuffer<'a> {
    type Target = [Insn<'a>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.inner, self.count) }
    }
}

impl<'a> Drop for InsnBuffer<'a> {
    fn drop(&mut self) {
        self.free();
    }
}

/// Iterator that lazily disassembles a binary blob of machine code.
/// This is constructed using [`crate::Capstone::disasm_iter`].
pub struct InsnIter<'a> {
    caps: &'a super::Capstone,
    insn: *mut Insn<'a>,
    code: *const u8,
    size: libc::size_t,
    addr: u64,
}

impl<'a> InsnIter<'a> {
    pub(crate) fn new(
        caps: &'a super::Capstone,
        insn: *mut Insn<'a>,
        code: *const u8,
        size: libc::size_t,
        addr: u64,
    ) -> InsnIter<'a> {
        InsnIter {
            caps,
            insn,
            code,
            size,
            addr,
        }
    }

    /// Frees the `Insn`(`cs_insn`) if it is not currently null
    /// then clears the pointer.
    fn free(&mut self) {
        if self.insn.is_null() {
            return;
        }
        unsafe { sys::cs_free(self.insn as *mut Insn, 1) };
        self.insn = core::ptr::null_mut();
    }
}

impl<'a> Iterator for InsnIter<'a> {
    type Item = Result<&'a Insn<'a>, super::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let success = unsafe {
            sys::cs_disasm_iter(
                self.caps.handle,
                &mut self.code,
                &mut self.size,
                &mut self.addr,
                self.insn,
            )
        };

        #[cfg(feature = "std")]
        self.caps.resume_panic();

        if !success {
            match self.caps.errno() {
                Ok(_) => return None,
                Err(err) => return Some(Err(err)),
            }
        }

        Ok(unsafe { self.insn.as_ref() }).transpose()
    }
}

impl<'a> Drop for InsnIter<'a> {
    fn drop(&mut self) {
        self.free();
    }
}

/// Extra details about an isntruction.
#[derive(Copy, Clone)]
pub struct Details<'i> {
    arch: Arch,
    inner: &'i DetailsInner,
}

impl<'i> Details<'i> {
    pub(crate) fn wrap(arch: Arch, inner: &'i DetailsInner) -> Details<'i> {
        Details { arch, inner }
    }

    /// Returns a list of registers that are **implicitly** read from by an instruction.
    /// For explicitly read registers, use the architecture specific details to access
    /// the operands of the instruction.
    pub fn regs_read(self) -> &'i [Reg] {
        unsafe {
            &*(&self.inner.regs_read[..self.inner.regs_read_count as usize] as *const [u16]
                as *const [Reg])
        }
    }

    /// Returns a list of registers that are **implicitly** written to by this instruction.
    /// For registers that are explicitly written to, use the architecture specific details
    /// to access the operands of the instruction.
    pub fn regs_write(self) -> &'i [Reg] {
        unsafe {
            &*(&self.inner.regs_write[..self.inner.regs_write_count as usize] as *const [u16]
                as *const [Reg])
        }
    }

    pub fn groups(self) -> &'i [InsnGroup] {
        unsafe {
            &*(&self.inner.groups[..self.inner.groups_count as usize] as *const [u8]
                as *const [InsnGroup])
        }
    }

    /// Returns architecture specific details.
    pub fn arch(self) -> ArchDetails<'i> {
        match self.arch {
            Arch::Arm => ArchDetails::Arm(unsafe { &self.inner.arch.arm }),
            Arch::Arm64 => ArchDetails::Arm64(unsafe { &self.inner.arch.arm64 }),
            Arch::Mips => ArchDetails::Mips(unsafe { &self.inner.arch.mips }),
            Arch::X86 => ArchDetails::X86(unsafe { &self.inner.arch.x86 }),
            Arch::PowerPc => ArchDetails::PowerPc(unsafe { &self.inner.arch.ppc }),
            Arch::Sparc => ArchDetails::Sparc(unsafe { &self.inner.arch.sparc }),
            Arch::SystemZ => ArchDetails::SystemZ(unsafe { &self.inner.arch.sysz }),
            Arch::XCore => ArchDetails::XCore(unsafe { &self.inner.arch.xcore }),
            Arch::M68K => ArchDetails::M68K(unsafe { &self.inner.arch.m68k }),
            Arch::Tms320C64X => ArchDetails::Tms320C64X(unsafe { &self.inner.arch.tms320c64x }),
            Arch::M680X => ArchDetails::M680X(unsafe { &self.inner.arch.m680x }),
            Arch::Evm => ArchDetails::Evm(unsafe { &self.inner.arch.evm }),
            Arch::Mos65xx => ArchDetails::Mos65xx(unsafe { &self.inner.arch.mos65xx }),
        }
    }

    /// If these are details for an x86 instruction, this will return
    /// x86 specific details. If these are not details for an x86 instruction
    /// this will return [`Option::None`].
    pub fn x86(self) -> Option<&'i x86::Details<'i>> {
        if self.arch == Arch::X86 {
            Some(unsafe { &self.inner.arch.x86 })
        } else {
            None
        }
    }
}

/// Wrapper around cs_detail.
#[repr(C)]
pub(crate) struct DetailsInner {
    /// List of implicit registers read by this insn.
    regs_read: [u16; 16],

    /// Number of implicit registers read by this insn.
    regs_read_count: u8,

    /// List of implicit registers modified by this insn.
    regs_write: [u16; 20],

    /// Number of implicit registers modified by this insn.
    regs_write_count: u8,

    /// List of group this instruction belong to.
    groups: [u8; 8],

    /// Number of groups this insn belongs to.
    groups_count: u8,

    /// Architecture specific details.
    pub(crate) arch: ArchDetailsUnion,
}

#[repr(C)]
pub(crate) union ArchDetailsUnion {
    pub x86: x86::Details<'static>,
    pub arm64: arm64::Details<'static>,
    pub arm: arm::Details<'static>,
    pub m68k: m68k::Details<'static>,
    pub mips: mips::Details<'static>,
    pub ppc: ppc::Details<'static>,
    pub sparc: sparc::Details<'static>,
    pub sysz: sysz::Details<'static>,
    pub xcore: xcore::Details<'static>,
    pub tms320c64x: tms320c64x::Details<'static>,
    pub m680x: m680x::Details<'static>,
    pub evm: evm::Details<'static>,
    pub mos65xx: mos65xx::Details<'static>,
}

#[derive(Copy, Clone)]
pub enum ArchDetails<'i> {
    X86(&'i x86::Details<'i>),
    Arm64(&'i arm64::Details<'i>),
    Arm(&'i arm::Details<'i>),
    M68K(&'i m68k::Details<'i>),
    Mips(&'i mips::Details<'i>),
    PowerPc(&'i ppc::Details<'i>),
    Sparc(&'i sparc::Details<'i>),
    SystemZ(&'i sysz::Details<'i>),
    XCore(&'i xcore::Details<'i>),
    Tms320C64X(&'i tms320c64x::Details<'i>),
    M680X(&'i m680x::Details<'i>),
    Evm(&'i evm::Details<'i>),
    Mos65xx(&'i mos65xx::Details<'i>),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn detail_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<DetailsInner>(),
            sys::get_test_val("sizeof(cs_detail)")
        );

        assert_eq!(
            core::mem::align_of::<DetailsInner>(),
            sys::get_test_val("alignof(cs_detail)")
        );
    }

    #[test]
    fn insn_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Insn>(),
            sys::get_test_val("sizeof(cs_insn)")
        );

        assert_eq!(
            core::mem::align_of::<Insn>(),
            sys::get_test_val("alignof(cs_insn)")
        );
    }
}
