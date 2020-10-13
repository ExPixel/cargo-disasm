use super::PackedCSInfo;
use crate::{arch, sys, util};
use core::marker::PhantomData;
use core::ptr::NonNull;

const MNEMONIC_SIZE: usize = 32;

/// Information about a disassembled instruction.
#[repr(transparent)]
pub struct Insn<'a> {
    inner: InsnInner,
    _phan: PhantomData<&'a InsnInner>,
}

impl<'a> Insn<'a> {
    /// Returns trhe address of this instruction.
    #[inline]
    pub fn address(&self) -> u64 {
        self.inner.address
    }

    /// Returns the size of this instruction in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.inner.size as usize
    }

    /// Returns the machine bytes of this instruction.
    /// The returned slice will have the same size as the return
    /// value of [`Insn::size`]
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.inner.bytes.as_ptr(), self.size()) }
    }

    /// Returns the instruction mnemonic.
    #[inline]
    pub fn mnemonic(&self) -> &str {
        unsafe { util::cstr(self.inner.mnemonic.as_ptr(), MNEMONIC_SIZE) }
    }

    /// Returns the instruction operands as a string.
    #[inline]
    pub fn operands(&self) -> &str {
        unsafe { util::cstr(self.inner.op_str.as_ptr(), 160) }
    }
}

/// A buffer of disassembled instructions.
pub struct InsnBuffer<'a> {
    inner: *mut Insn<'a>,
    count: usize,
    _phan: PhantomData<&'a Insn<'a>>,
}

impl<'a> InsnBuffer<'a> {
    fn free(&mut self) {
        if self.inner.is_null() {
            return;
        }
        unsafe { sys::cs_free(self.inner as *mut InsnInner, self.count as libc::size_t) };
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

/// Wrapper around cs_insn.
#[repr(C)]
pub struct InsnInner {
    /// Instruction ID (basically a numeric ID for the instruction mnemonic)
    /// Find the instruction id in the '[ARCH]_insn' enum in the header file
    /// of corresponding architecture, such as 'arm_insn' in arm.h for ARM,
    /// 'x86_insn' in x86.h for X86, etc...
    /// This information is available even when CS_OPT_DETAIL = CS_OPT_OFF
    /// NOTE: in Skipdata mode, "data" instruction has 0 for this id field.
    id: libc::c_uint,

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
    detail: Option<NonNull<DetailInner>>,
}

/// Wrapper around cs_detail.
#[repr(C)]
struct DetailInner {
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
    arch: DetailsInnerArch,
}

#[repr(C)]
union DetailsInnerArch {
    x86: arch::x86::X86DetailsInner,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn detail_size_and_alignment() {
        assert_eq!(core::mem::size_of::<DetailInner>(), unsafe {
            sys::ep_helper__sizeof_cs_detail() as usize
        });

        assert_eq!(core::mem::align_of::<DetailInner>(), unsafe {
            sys::ep_helper__alignof_cs_detail() as usize
        });
    }

    #[test]
    fn insn_size_and_alignment() {
        assert_eq!(core::mem::size_of::<InsnInner>(), unsafe {
            sys::ep_helper__sizeof_cs_insn() as usize
        });

        assert_eq!(core::mem::align_of::<InsnInner>(), unsafe {
            sys::ep_helper__alignof_cs_detail() as usize
        });
    }
}
