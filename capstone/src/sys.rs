use core::ptr::NonNull;

extern "C" {
    pub fn cs_version(major: *mut libc::c_int, minor: *mut libc::c_int) -> libc::c_int;
    pub fn cs_support(query: libc::c_int) -> bool;
    pub fn cs_open(arch: Arch, mode: Mode, csh: *mut Handle) -> Error;
    pub fn cs_close(handle: *mut Handle) -> Error;
    pub fn cs_option(handle: Handle, type_: OptType, value: libc::size_t) -> Error;
    pub fn cs_malloc<'s>(handle: Handle) -> *mut crate::insn::Insn<'s>;
    pub fn cs_free(insn: *mut crate::insn::Insn, count: libc::size_t);
    pub fn cs_errno(handle: Handle) -> Error;

    pub fn cs_disasm(
        handle: Handle,
        code: *const u8,
        code_size: libc::size_t,
        address: u64,
        count: libc::size_t,
        insn: *mut *mut crate::insn::Insn,
    ) -> libc::size_t;

    pub fn cs_disasm_iter(
        handle: Handle,
        code: *mut *const u8,
        size: *mut libc::size_t,
        address: *mut u64,
        insn: *mut crate::insn::Insn,
    ) -> bool;
}

#[cfg(test)]
extern "C" {
    fn ep_helper__get_value(name: *const libc::c_char, len: libc::size_t) -> libc::size_t;
}

#[cfg(test)]
pub fn get_test_val(name: &str) -> usize {
    unsafe {
        ep_helper__get_value(
            name.as_ptr() as *const libc::c_char,
            name.len() as libc::size_t,
        ) as usize
    }
}

pub type SkipdataCallback = extern "C" fn(
    code: *mut u8,
    code_size: *mut libc::size_t,
    offset: libc::size_t,
    user_data: *mut libc::c_void,
) -> libc::size_t;

#[repr(C)]
pub struct OptSkipdataSetup {
    pub mnemonic: Option<NonNull<libc::c_char>>,
    pub callback: Option<SkipdataCallback>,
    pub userdata: *mut libc::c_void,
}

#[repr(C)]
pub struct OptMnemonic {
    pub id: libc::c_int,
    pub mnemonic: *const libc::c_char,
}

#[repr(C)]
pub enum OptType {
    /// No option specified.
    #[allow(dead_code)]
    Invalid = 0,

    /// Assembly output syntax.
    Syntax,
    /// Break down instruction structure into details.
    Detail,
    /// Change engine's mode at runtime.
    Mode,

    /// User-defined dynamic memory related functions.
    #[allow(dead_code)]
    Mem,

    /// Skipdata when disassembling. This places the engine
    /// in SKIPDATA mode.
    Skipdata,
    /// Setup user-defined function for SKIPDATA mode.
    SkipdataSetup,
    /// Customize instruction mnemonic.
    Mnemonic,
    /// Print immediate operands in unsigned form.
    Unsigned,
}

/// Turn OFF an option.
pub const OPT_VALUE_OFF: libc::size_t = 0;
/// Turn ON an option.
pub const OPT_VALUE_ON: libc::size_t = 3;
/// Default ASM syntax.
pub const OPT_VALUE_SYNTAX_DEFAULT: libc::size_t = 0;
/// X86 Intel ASM syntax.
pub const OPT_VALUE_SYNTAX_INTEL: libc::size_t = 1;
/// X86 AT&T ASM syntax.
pub const OPT_VALUE_SYNTAX_ATT: libc::size_t = 2;
/// Print register names as a number.
pub const OPT_VALUE_SYNTAX_NOREGNAME: libc::size_t = 3;
/// X86 Intel MASM syntax.
pub const OPT_VALUE_SYNTAX_MASM: libc::size_t = 4;

/// Transparent wrapper for `cs_arch`.
#[repr(transparent)]
pub struct Arch(pub libc::c_int);

/// Transparent wrapper for `cs_mode`.
#[repr(transparent)]
pub struct Mode(pub libc::c_int);

/// Transparent wrapper for `csh`.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Handle(pub libc::size_t);

/// Transparent wrapper for `cs_err`.
#[repr(transparent)]
pub struct Error(pub libc::c_int);
