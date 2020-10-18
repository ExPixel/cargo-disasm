use crate::arch::{
    arm, arm64, evm, m680x, m68k, mips, mos65xx, ppc, sparc, sysz, tms320c64x, x86, xcore,
};
use crate::{sys, util};
use core::marker::PhantomData;
use core::ptr::NonNull;

const MNEMONIC_SIZE: usize = 32;

/// Information about a disassembled instruction.
#[repr(C)]
pub struct Insn<'a> {
    /// Instruction ID (basically a numeric ID for the instruction mnemonic)
    /// Find the instruction id in the '[ARCH]_insn' enum in the header file
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
    pub(crate) detail: Option<NonNull<Details>>,

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
                Ok(_) => return Some(Err(super::Error::Bindings)),
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

/// Wrapper around cs_detail.
#[repr(C)]
pub struct Details {
    /// List of implicit registers read by this insn.
    regs_read: [u16; 16],

    /// Number of implicit registers read by this insn.
    regs_read_count: u8,

    /// List of implicit registers modified by this insn.
    reads_write: [u16; 20],

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
    pub x86: x86::Details,
    pub arm64: arm64::Details,
    pub arm: arm::Details,
    pub m68k: m68k::Details,
    pub mips: mips::Details,
    pub ppc: ppc::Details,
    pub sparc: sparc::Details,
    pub sysz: sysz::Details,
    pub xcore: xcore::Details,
    pub tms320c64x: tms320c64x::Details,
    pub m680x: m680x::Details,
    pub evm: evm::Details,
    pub mos65xx: mos65xx::Details,
}

pub enum ArchDetails<'i> {
    X86(&'i x86::Details),
    Arm64(&'i arm64::Details),
    Arm(&'i arm::Details),
    M68K(&'i m68k::Details),
    Mips(&'i mips::Details),
    PowerPc(&'i ppc::Details),
    Sparc(&'i sparc::Details),
    SystemZ(&'i sysz::Details),
    XCore(&'i xcore::Details),
    Tms320C64X(&'i tms320c64x::Details),
    M680X(&'i m680x::Details),
    Evm(&'i evm::Details),
    Mos65xx(&'i mos65xx::Details),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn detail_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_detail)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
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
