#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

#[macro_use]
mod macros;
pub mod arch;
mod insn;
mod sys;
mod util;

use core::{
    convert::{From, TryFrom},
    fmt,
    ptr::NonNull,
};

#[cfg(feature = "std")]
use std::{self as alloc, cell::RefCell, panic::UnwindSafe};

pub use arch::InsnId;
pub use insn::{Insn, InsnBuffer, InsnIter};

#[doc(inline)]
pub use arch::arm;
#[doc(inline)]
pub use arch::arm64;
#[doc(inline)]
pub use arch::evm;
#[doc(inline)]
pub use arch::m680x;
#[doc(inline)]
pub use arch::m68k;
#[doc(inline)]
pub use arch::mips;
#[doc(inline)]
pub use arch::mos65xx;
#[doc(inline)]
pub use arch::ppc;
#[doc(inline)]
pub use arch::sparc;
#[doc(inline)]
pub use arch::sysz;
#[doc(inline)]
pub use arch::tms320c64x;
#[doc(inline)]
pub use arch::x86;
#[doc(inline)]
pub use arch::xcore;

#[cfg(feature = "std")]
pub type SkipdataCallback = dyn 'static + UnwindSafe + FnMut(&[u8], usize) -> usize;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
pub type SkipdataCallback = dyn 'static + FnMut(&[u8], usize) -> usize;

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub type SkipdataCallback = fn(&[u8], usize) -> usize;

/// A capstone instance that can be used for disassembly.
pub struct Capstone {
    handle: sys::Handle,
    packed: PackedCSInfo,
    #[cfg(feature = "alloc")]
    skipdata_callback: Option<alloc::boxed::Box<SkipdataCallback>>,

    #[cfg(feature = "alloc")]
    skipdata_mnemonic: Option<alloc::borrow::Cow<'static, str>>,

    #[cfg(not(feature = "alloc"))]
    skipdata_callback: Option<SkipdataCallback>,

    #[cfg(feature = "std")]
    pending_panic: RefCell<Option<Box<dyn std::any::Any + Send + 'static>>>,
}

impl Capstone {
    /// Initializes capstone with the given arch and mode.
    pub fn open(arch: Arch, mode: Mode) -> Result<Self, Error> {
        let mut handle = sys::Handle(0);

        result! {
            unsafe { sys::cs_open(arch.into(), mode.into(), &mut handle) },
            Capstone {
                handle,
                packed: PackedCSInfo::new(arch, false, false),
                skipdata_callback: None,

                #[cfg(feature = "alloc")]
                skipdata_mnemonic: None,

                #[cfg(feature = "std")]
                pending_panic: RefCell::new(None),
            }
        }
    }

    /// Reports the last error that occurred in the API after a function
    /// has failed. Like glibc's errno, this might not retain its old value
    /// once it has been accessed.
    fn errno(&self) -> Result<(), Error> {
        result!(unsafe { sys::cs_errno(self.handle) })
    }

    /// Disassembles all of the instructions in a buffer with the given
    /// starting address. This will dynamically allocate memory to
    /// contain the disassembled instructions.
    pub fn disasm<'s>(&'s self, code: &[u8], address: u64) -> Result<InsnBuffer<'s>, Error> {
        self.priv_disasm(code, address, 0)
    }

    /// Disassembles at most `count` instructions from the buffer using
    /// the given starting address. This will dynamically allocate memory
    /// to contain the disassembled instructions.
    pub fn disasm_count<'s>(
        &'s self,
        code: &[u8],
        address: u64,
        count: usize,
    ) -> Result<InsnBuffer<'s>, Error> {
        if count == 0 {
            Ok(InsnBuffer::new(NonNull::dangling().as_ptr(), 0))
        } else {
            self.priv_disasm(code, address, count)
        }
    }

    /// Disassembles a binary given a buffer, a starting address, and the number
    /// of instructions to disassemble. If `count` is `0`, this will disassbmle
    /// all of the instructiosn in the buffer. This API will dynamically allocate
    /// memory to contain the disassembled instructions.
    fn priv_disasm<'s>(
        &'s self,
        code: &[u8],
        address: u64,
        count: usize,
    ) -> Result<InsnBuffer<'s>, Error> {
        let mut insn: *mut Insn = core::ptr::null_mut();

        // the real count
        let count = unsafe {
            sys::cs_disasm(
                self.handle,
                code.as_ptr(),
                code.len() as libc::size_t,
                address,
                count as libc::size_t,
                &mut insn,
            )
        } as usize;

        #[cfg(feature = "std")]
        self.resume_panic();

        if count == 0 {
            self.errno()?;
            return Err(Error::Bindings);
        }

        Ok(InsnBuffer::new(insn, count))
    }

    /// Returns an iterator that will lazily disassemble the instructions
    /// in the given binary.
    pub fn disasm_iter<'s>(&'s self, code: &[u8], address: u64) -> InsnIter<'s> {
        let insn = unsafe { sys::cs_malloc(self.handle) };
        assert!(!insn.is_null(), "cs_malloc() returned a null insn");

        InsnIter::new(
            self,
            insn,
            code.as_ptr(),
            code.len() as libc::size_t,
            address,
        )
    }

    /// Sets the assembly syntax for the disassembling engine at runtime.
    ///
    /// If the syntax is supported then [`Result::Ok`] is returned
    /// with no value. If the syntax is not supported then [`Result::Err`]
    /// is returned.
    pub fn set_syntax(&mut self, syntax: Syntax) -> Result<(), Error> {
        match syntax {
            Syntax::Default => self.set_option(sys::OptType::Syntax, sys::OPT_VALUE_SYNTAX_DEFAULT),
            Syntax::Intel => self.set_option(sys::OptType::Syntax, sys::OPT_VALUE_SYNTAX_INTEL),
            Syntax::Att => self.set_option(sys::OptType::Syntax, sys::OPT_VALUE_SYNTAX_ATT),
            Syntax::NoRegName => {
                self.set_option(sys::OptType::Syntax, sys::OPT_VALUE_SYNTAX_NOREGNAME)
            }
            Syntax::Masm => self.set_option(sys::OptType::Syntax, sys::OPT_VALUE_SYNTAX_MASM),
        }
    }

    /// Change the engine's mode at runtime after it has been initialized.
    pub fn set_mode(&mut self, mode: Mode) -> Result<(), Error> {
        self.set_option(sys::OptType::Mode, mode.bits() as libc::size_t)
    }

    /// Setting `detail` to true will make the disassembling engine break
    /// down instruction structure into details.
    pub fn set_detail(&mut self, detail: bool) -> Result<(), Error> {
        self.set_option(
            sys::OptType::Detail,
            if detail {
                sys::OPT_VALUE_ON
            } else {
                sys::OPT_VALUE_OFF
            },
        )?;

        self.packed.set_detail(detail);
        Ok(())
    }

    /// Setting `unsigned` to true will make the disassembling engine print
    /// immediate operands in unsigned form.
    pub fn set_unsigned(&mut self, unsigned: bool) -> Result<(), Error> {
        self.set_option(
            sys::OptType::Unsigned,
            if unsigned {
                sys::OPT_VALUE_ON
            } else {
                sys::OPT_VALUE_OFF
            },
        )?;
        Ok(())
    }

    /// Customize the mnemonic for an instruction with an alternative name.
    pub fn set_mnemonic(&mut self, insn: InsnId, mnemonic: &'static str) -> Result<(), Error> {
        let mut opt_mnem = sys::OptMnemonic {
            id: insn.into(),
            mnemonic: mnemonic.as_ptr() as *const libc::c_char,
        };

        self.set_option(
            sys::OptType::Mnemonic,
            &mut opt_mnem as *mut _ as usize as libc::size_t,
        )
    }

    /// Sets a custom setup for SKIPDATA mode.
    ///
    /// Setting mnemonic allows for customizing the mnemonic of the instruction
    /// used to represent data. By default this will be `.byte`.
    ///
    /// The user defined callback (if there is one) will be called whenever
    /// Capstone hits data. If the returned value from the callback is positive (greater than `0`), Capstone
    /// will skip exactly that number of bytes and continue. Otherwise, if the callback retruns `0`,
    /// Capstone stops disassembling and returns immediately from [`Capstone::disasm`] or causes
    /// the [`Iterator`] from [`Capstone::disasm_iter`] to return [`None`].
    ///
    /// # Note
    ///
    /// If the callback is `None`, Capstone will skip a number of bytes depending on the
    /// architecture:
    ///
    /// * Arm:     2 bytes (Thumb mode) or 4 bytes.
    /// * Arm64:   4 bytes.
    /// * Mips:    4 bytes.
    /// * M680x:   1 byte.
    /// * PowerPC: 4 bytes.
    /// * Sparc:   4 bytes.
    /// * SystemZ: 2 bytes.
    /// * X86:     1 bytes.
    /// * XCore:   2 bytes.
    /// * EVM:     1 bytes.
    /// * MOS65XX: 1 bytes.
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    pub fn setup_skipdata<M, F>(
        &mut self,
        mnemonic: Option<M>,
        callback: Option<F>,
    ) -> Result<(), Error>
    where
        M: Into<alloc::borrow::Cow<'static, str>>,
        F: 'static + FnMut(&[u8], usize) -> usize,
    {
        self.skipdata_mnemonic = mnemonic.map(|m| m.into());
        self.skipdata_callback = callback.map(|c| alloc::boxed::Box::new(c) as _);

        let setup = sys::OptSkipdataSetup {
            mnemonic: self
                .skipdata_mnemonic
                .as_ref()
                .map(|m| unsafe { NonNull::new_unchecked((&*m).as_ptr() as *mut libc::c_char) }),
            callback: self.skipdata_callback.as_ref().map(|_| cs_skipdata_cb as _),
            userdata: self as *mut Self as *mut libc::c_void,
        };

        self.set_option(
            sys::OptType::SkipdataSetup,
            &setup as *const _ as usize as libc::size_t,
        )?;
        Ok(())
    }

    /// Sets a custom setup for SKIPDATA mode.
    ///
    /// Setting mnemonic allows for customizing the mnemonic of the instruction
    /// used to represent data. By default this will be `.byte`.
    ///
    /// The user defined callback (if there is one) will be called whenever
    /// Capstone hits data. If the returned value from the callback is positive (greater than `0`), Capstone
    /// will skip exactly that number of bytes and continue. Otherwise, if the callback retruns `0`,
    /// Capstone stops disassembling and returns immediately from [`Capstone::disasm`] or causes
    /// the [`Iterator`] from [`Capstone::disasm_iter`] to return [`None`].
    ///
    /// # Note
    ///
    /// If the callback is `None`, Capstone will skip a number of bytes depending on the
    /// architecture:
    ///
    /// * Arm:     2 bytes (Thumb mode) or 4 bytes.
    /// * Arm64:   4 bytes.
    /// * Mips:    4 bytes.
    /// * M680x:   1 byte.
    /// * PowerPC: 4 bytes.
    /// * Sparc:   4 bytes.
    /// * SystemZ: 2 bytes.
    /// * X86:     1 bytes.
    /// * XCore:   2 bytes.
    /// * EVM:     1 bytes.
    /// * MOS65XX: 1 bytes.
    #[cfg(feature = "std")]
    pub fn setup_skipdata<M, F>(
        &mut self,
        mnemonic: Option<M>,
        callback: Option<F>,
    ) -> Result<(), Error>
    where
        M: Into<alloc::borrow::Cow<'static, str>>,
        F: 'static + UnwindSafe + FnMut(&[u8], usize) -> usize,
    {
        self.skipdata_mnemonic = mnemonic.map(|m| m.into());
        self.skipdata_callback = callback.map(|c| alloc::boxed::Box::new(c) as _);

        let setup = sys::OptSkipdataSetup {
            mnemonic: self
                .skipdata_mnemonic
                .as_ref()
                .map(|m| unsafe { NonNull::new_unchecked((&*m).as_ptr() as *mut libc::c_char) }),
            callback: self.skipdata_callback.as_ref().map(|_| cs_skipdata_cb as _),
            userdata: self as *mut Self as *mut libc::c_void,
        };

        self.set_option(
            sys::OptType::SkipdataSetup,
            &setup as *const _ as usize as libc::size_t,
        )?;
        Ok(())
    }

    /// Sets a custom setup for SKIPDATA mode.
    ///
    /// Setting mnemonic allows for customizing the mnemonic of the instruction
    /// used to represent data. By default this will be `.byte`.
    ///
    /// The user defined callback (if there is one) will be called whenever
    /// Capstone hits data. If the returned value from the callback is positive (greater than `0`), Capstone
    /// will skip exactly that number of bytes and continue. Otherwise, if the callback retruns `0`,
    /// Capstone stops disassembling and returns immediately from [`Capstone::disasm`] or causes
    /// the [`Iterator`] from [`Capstone::disasm_iter`] to return [`None`].
    ///
    /// # Note
    ///
    /// If the callback is `None`, Capstone will skip a number of bytes depending on the
    /// architecture:
    ///
    /// * Arm:     2 bytes (Thumb mode) or 4 bytes.
    /// * Arm64:   4 bytes.
    /// * Mips:    4 bytes.
    /// * M680x:   1 byte.
    /// * PowerPC: 4 bytes.
    /// * Sparc:   4 bytes.
    /// * SystemZ: 2 bytes.
    /// * X86:     1 bytes.
    /// * XCore:   2 bytes.
    /// * EVM:     1 bytes.
    /// * MOS65XX: 1 bytes.
    #[cfg(not(feature = "alloc"))]
    pub fn setup_skipdata<M, F>(
        &mut self,
        mnemonic: Option<&'static str>,
        callback: Option<fn(&[u8], usize) -> usize>,
    ) -> Result<(), Error> {
        self.skipdata_callback = callback;

        let setup = sys::OptSkipdataSetup {
            mnemonic: mnemonic
                .map(|m| unsafe { NonNull::new_unchecked(m.as_ptr() as *mut libc::c_char) }),
            callback: self.skipdata_callback.as_ref().map(|_| cs_skipdata_cb as _),
            userdata: self as *mut Self as *mut libc::c_void,
        };

        self.set_option(
            sys::OptType::SkipdataSetup,
            &setup as *const _ as usize as libc::size_t,
        )?;
        Ok(())
    }

    /// If there is a panic waiting in [`Capstone::pending_panic`], this will
    /// resume it.
    #[cfg(feature = "std")]
    fn resume_panic(&self) {
        if self.pending_panic.borrow().is_none() {
            return;
        }

        if let Some(p) = self.pending_panic.borrow_mut().take() {
            std::panic::resume_unwind(p);
        }
    }

    /// Place the disassembling engine in SKIPDATA mode.
    /// Use [`Capstone::setup_skipdata`] to configure this mode.
    pub fn set_skipdata(&mut self, skipdata: bool) -> Result<(), Error> {
        self.set_option(
            sys::OptType::Skipdata,
            if skipdata {
                sys::OPT_VALUE_ON
            } else {
                sys::OPT_VALUE_OFF
            },
        )?;

        self.packed.set_skipdata(skipdata);
        Ok(())
    }

    /// Returns true if `detail` is set for the disassembling engine.
    pub fn detail(&self) -> bool {
        self.packed.detail()
    }

    /// Returns true if the disassembling engine is currently in SKIPDATA
    /// mode.
    pub fn skipdata(&self) -> bool {
        self.packed.skipdata()
    }

    /// Returns the current arch that this instance of the Capstone
    /// disassembly engine is set to disassemble.
    pub fn arch(&self) -> Arch {
        self.packed.arch()
    }

    /// Set an option for the disassembling engine at runtime.
    fn set_option(&mut self, type_: sys::OptType, value: libc::size_t) -> Result<(), Error> {
        result!(unsafe { sys::cs_option(self.handle, type_, value) })
    }

    /// Closes the capstone handle.
    ///
    /// # Panics
    ///
    /// Panics if an error occurs while closing the CS handle.
    /// The only possible error really is an invalid handle, in which case
    /// something has gone very wrong in the bindings.
    fn close(&mut self) {
        result!(unsafe { sys::cs_close(&mut self.handle) })
            .expect("error occurred while closing Capstone handle");
    }
}

impl Drop for Capstone {
    fn drop(&mut self) {
        self.close();
    }
}

extern "C" fn cs_skipdata_cb(
    code: *mut u8,
    code_size: *mut libc::size_t,
    offset: libc::size_t,
    userdata: *mut libc::c_void,
) -> libc::size_t {
    if userdata.is_null() {
        return 0;
    }
    let userdata = userdata as *mut Capstone;

    #[cfg(feature = "std")]
    unsafe {
        // Don't allow any callbacks to be used again if there is a panic
        // that has not yet been handled.
        if (*userdata).pending_panic.borrow().is_some() {
            return 0;
        }

        // SAFETY: If a panic occurs we never use this closure again.
        //         Although I might be misunderstanding unwind safety here (- Marc)
        let cb = std::panic::AssertUnwindSafe(&mut (*userdata).skipdata_callback);

        match std::panic::catch_unwind(move || {
            if let std::panic::AssertUnwindSafe(Some(ref mut cb)) = cb {
                cb(
                    core::slice::from_raw_parts_mut(code, code_size as usize),
                    offset as usize,
                )
            } else {
                // This should technically be unreachable.
                0
            }
        }) {
            Ok(ret) => ret as libc::size_t,
            Err(p) => {
                *(*userdata).pending_panic.borrow_mut() = Some(p);
                0
            }
        }
    }

    // The no_std and no_std+alloc version of these can just share the same code.
    #[cfg(not(feature = "std"))]
    unsafe {
        if let Some(ref mut cb) = (*userdata).skipdata_callback {
            cb(
                core::slice::from_raw_parts_mut(code, code_size as usize),
                offset as usize,
            ) as libc::size_t
        } else {
            // This should be unreachable.
            0
        }
    }
}

/// Disassembling engine assembly syntax.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum Syntax {
    Default,
    /// Intel assembly syntax.
    Intel,
    /// AT&T assembly syntax.
    Att,
    /// Print register names as numbers.
    NoRegName,
    /// Intel MASM assembly syntax.
    Masm,
}

impl Default for Syntax {
    fn default() -> Self {
        Self::Default
    }
}

/// The API version of capstone.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct CapstoneVersion {
    /// The major version of capstone.
    pub major: u16,
    /// The minor version of capstone.
    pub minor: u16,
}

impl fmt::Display for CapstoneVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

c_enum! {
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum Arch: u8 + i32 + u32 {
        /// ARM architecture (including Thumb, Thumb-2)
        Arm,
        /// ARM-64, also called AArch64
        Arm64,
        /// Mips architecture
        Mips,
        /// X86 architecture (including x86 & x86-64)
        X86,
        /// PowerPC architecture
        PowerPC,
        /// Sparc architecture
        Sparc,
        /// SystemZ architecture
        SystemZ,
        /// XCore architecture
        XCore,
        /// 68K architecture
        M68K,
        /// TMS320C64x architecture
        Tms320C64X,
        /// 680X architecture
        M680X,
        /// Ethereum architecture
        Evm,
        /// MOS65XX architecture (including MOS6502)
        Mos65XX,
    }
}

impl From<Arch> for sys::Arch {
    fn from(arch: Arch) -> sys::Arch {
        sys::Arch(arch.into())
    }
}

/// Support query that can be used along with `supports` to check
/// the current Capstone build's capabilities.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum SupportQuery {
    /// Support query for a specific architecture.
    Arch(Arch),

    /// Support query for all architectures known to capstone.
    AllArch,

    /// Support query for verifying that the current capstone
    /// engine is in diet mode.
    Diet,

    /// Support query for verifying that the current capstone
    /// engine is currently in X86 reduce mode.
    X86Reduce,
}

impl From<Arch> for SupportQuery {
    fn from(arch: Arch) -> SupportQuery {
        SupportQuery::Arch(arch)
    }
}

#[allow(non_upper_case_globals)]
mod mode {
    bitflags::bitflags! {
        /// Mode flags for configuring `Capstone`.
        pub struct Mode: libc::c_int {
            /// little-endian mode (default mode)
            const LittleEndian = 0;
            /// 32-bit ARM
            const Arm = 0;
            /// 16-bit mode (X86)
            const Bits16 = 1 << 1;
            /// 32-bit mode (X86)
            const Bits32 = 1 << 2;
            /// 64-bit mode (X86, PPC)
            const Bits64 = 1 << 3;
            /// ARM's Thumb mode, including Thumb-2
            const Thumb = 1 << 4;
            /// ARM's Cortex-M series
            const MClass = 1 << 5;
            /// ARMv8 A32 encodings for ARM
            const V8 = 1 << 6;
            /// MicroMips mode (MIPS)
            const Micro = 1 << 4;
            /// MIPS III ISA
            const Mips3 = 1 << 5;
            /// MIPS32R6 ISA
            const Mips32R6 = 1 << 6;
            /// Mips II ISA
            const Mips2 = 1 << 7;
            /// SparcV9 mode (Sparc)
            const V9 = 1 << 4;
            /// Quad Processing eXtensions mode (PPC)
            const Qpx = 1 << 4;
            /// M68K 68000 mode
            const M68K000 = 1 << 1;
            /// M68K 68010 mode
            const M68K010 = 1 << 2;
            /// M68K 68020 mode
            const M68K020 = 1 << 3;
            /// M68K 68030 mode
            const M68K030 = 1 << 4;
            /// M68K 68040 mode
            const M68K040 = 1 << 5;
            /// M68K 68060 mode
            const M68K060 = 1 << 6;
            /// big-endian mode
            const BigEndian = 1 << 31;
            /// MIPS32 ISA (Mips)
            const Mips32 = Self::Bits32.bits;
            /// MIPS64 ISA (Mips)
            const Mips64 = Self::Bits64.bits;
            /// M680X Hitachi 6301,6303 mode
            const M680X6301 = 1 << 1;
            /// M680X Hitachi 6309 mode
            const M680X6309 = 1 << 2;
            /// M680X Motorola 6800,6802 mode
            const M680X6800 = 1 << 3;
            /// M680X Motorola 6801,6803 mode
            const M680X6801 = 1 << 4;
            /// M680X Motorola/Freescale 6805 mode
            const M680X6805 = 1 << 5;
            /// M680X Motorola/Freescale/NXP 68HC08 mode
            const M680X6808 = 1 << 6;
            /// M680X Motorola 6809 mode
            const M680X6809 = 1 << 7;
            /// M680X Motorola/Freescale/NXP 68HC11 mode
            const M680X6811 = 1 << 8;
            /// M680X Motorola/Freescale/NXP CPU12 used on M68HC12/HCS12
            const M680XCPU12 = 1 << 9;
            /// M680X Freescale/NXP HCS08 mode
            const M680XHCS08 = 1 << 10;
        }
    }
}

#[doc(inline)]
pub use mode::Mode;

impl From<Mode> for sys::Mode {
    fn from(mode: Mode) -> sys::Mode {
        sys::Mode(mode.bits() as _)
    }
}

c_enum! {
    #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
    pub enum Error: u8 + i32 + u32 {
        /// Out of memory error.
        Memory = 1,
        /// Unsupported architecture.
        Arch,
        /// Invalid handle.
        Handle,
        /// Invalid Capstone handle argument.
        ///
        /// **NOTE**: This should not come up using the safe bindings. If
        /// it does please file an issue.
        Csh,
        /// Invalid/unsupported mode.
        Mode,
        /// Invalid/unsupported option.
        Option,
        /// Information is unavailable because detail option is OFF.
        Detail,
        /// Dynamic memory management uninitialized.
        MemSetup,
        /// Unsupported version (bindings).
        Version,
        /// Accessed irrelevant data in "diet" engine.
        Diet,
        /// Accessed irrelevant data for "data" instruction in SKIPDATA mode.
        Skipdata,
        /// X86 AT&T syntax is unsupported (opted out at compile time).
        X86Att,
        /// X86 Intel syntex is unsupported (opted out at compile time).
        X86Intel,
        /// X86 MASM syntex is unsupported (opted out at compile time).
        X86Masm,
        /// An error occurred in the bindings. Truly terrible.
        Bindings,
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Error::Memory => "out of memory",
            Error::Arch => "unsupported architecture",
            Error::Handle => "invalid handle",
            Error::Csh => "invalid capstone handle",
            Error::Mode => "invalid/unsupported mode",
            Error::Option => "invalid/unsupported option",
            Error::Detail => "detail unavailable",
            Error::MemSetup => "dynamic memory management uninitialized",
            Error::Version => "unsupported version",
            Error::Diet => "accessed irrelevant data in diet engine",
            Error::Skipdata => "accessed irrelevant data for data instruction in skipdata mode",
            Error::X86Att => "X86 AT&T syntax is unsupported",
            Error::X86Intel => "X86 Intel syntex is unsupported",
            Error::X86Masm => "X86 MASM syntex is unsupported",
            Error::Bindings => "bindings error (please file an issue)",
        };

        f.write_str(msg)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Packed information about a current instance of capstone.
///
/// The bits are packed in this format:
/// value       start   end     range
/// arch        0       3       0-15
/// detail      4       4       1
/// skipdata    5       5       1
#[derive(Clone, Copy)]
struct PackedCSInfo(u8);

impl PackedCSInfo {
    fn new(arch: Arch, detail: bool, skipdata: bool) -> Self {
        let mut p = PackedCSInfo(0);
        p.set_arch(arch);
        p.set_detail(detail);
        p.set_skipdata(skipdata);
        p
    }

    fn arch(self) -> Arch {
        match Arch::try_from(self.0 & 0xF) {
            Ok(arch) => arch,

            #[cfg(test)]
            Err(_) => unreachable!("bad arch from PackedCSInfo"),

            // SAFETY: we never allow an invalid Arch to be set on PackedCSInfo.
            #[cfg(not(test))]
            Err(_) => unsafe { core::hint::unreachable_unchecked() },
        }
    }

    fn detail(self) -> bool {
        ((self.0 >> 4) & 1) != 0
    }

    fn skipdata(self) -> bool {
        ((self.0 >> 5) & 1) != 0
    }

    fn set_arch(&mut self, arch: Arch) {
        self.0 = (self.0 & !0xF) | u8::from(arch)
    }

    fn set_detail(&mut self, detail: bool) {
        self.0 = (self.0 & !(1 << 4)) | ((detail as u8) << 4);
    }

    fn set_skipdata(&mut self, skipdata: bool) {
        self.0 = (self.0 & !(1 << 5)) | ((skipdata as u8) << 5);
    }
}

/// Returns the current version of the capstone API.
pub fn version() -> CapstoneVersion {
    let mut major: libc::c_int = 0;
    let mut minor: libc::c_int = 0;
    unsafe { sys::cs_version(&mut major, &mut minor) };
    CapstoneVersion {
        major: major as u16,
        minor: minor as u16,
    }
}

/// Queries Capstone's capabilities. Use this to check if the current build of
/// Capstone supports a certain architecture or if the feature set is reduced.
pub fn supports<Query>(query: Query) -> bool
where
    Query: Into<SupportQuery>,
{
    let query_int = match query.into() {
        SupportQuery::Arch(arch) => arch as libc::c_int,
        SupportQuery::AllArch => 0xFFFF,
        SupportQuery::Diet => 0x10000,
        SupportQuery::X86Reduce => 0x10001,
    };
    unsafe { sys::cs_support(query_int) }
}

#[cfg(test)]
mod test {
    use super::*;

    const ALL_ARCHS: &[Arch] = &[
        Arch::Arm,
        Arch::Arm64,
        Arch::Mips,
        Arch::X86,
        Arch::PowerPC,
        Arch::Sparc,
        Arch::SystemZ,
        Arch::XCore,
        Arch::M68K,
        Arch::Tms320C64X,
        Arch::M680X,
        Arch::Evm,
        Arch::Mos65XX,
    ];

    #[test]
    fn open_capstone() {
        let caps = Capstone::open(Arch::X86, Mode::LittleEndian).expect("failed to open capstone");

        for insn in caps.disasm(&[0xcc], 0x0).unwrap().iter() {
            println!("{} {}", insn.mnemonic(), insn.operands());
        }
    }

    #[test]
    fn validate_packed_cs_info_states() {
        for arch in ALL_ARCHS.iter().copied() {
            let packed = PackedCSInfo::new(arch, true, true);
            assert_eq!(packed.arch(), arch);
            assert_eq!(packed.detail(), true);
            assert_eq!(packed.skipdata(), true);

            let packed = PackedCSInfo::new(arch, false, true);
            assert_eq!(packed.arch(), arch);
            assert_eq!(packed.detail(), false);
            assert_eq!(packed.skipdata(), true);

            let packed = PackedCSInfo::new(arch, true, false);
            assert_eq!(packed.arch(), arch);
            assert_eq!(packed.detail(), true);
            assert_eq!(packed.skipdata(), false);

            let packed = PackedCSInfo::new(arch, false, false);
            assert_eq!(packed.arch(), arch);
            assert_eq!(packed.detail(), false);
            assert_eq!(packed.skipdata(), false);
        }
    }

    #[test]
    fn test_version() {
        pub const EXPECTED_MAJOR_VERSION: u16 = 5;
        pub const EXPECTED_MINOR_VERSION: u16 = 0;

        let v = version();
        assert_eq!(v.major, EXPECTED_MAJOR_VERSION);
        assert_eq!(v.minor, EXPECTED_MINOR_VERSION);
    }

    #[test]
    fn test_support() {
        assert_eq!(supports(Arch::Arm), cfg!(feature = "arm"));
        assert_eq!(supports(Arch::Arm64), cfg!(feature = "aarch64"));
        assert_eq!(supports(Arch::Mips), cfg!(feature = "mips"));
        assert_eq!(supports(Arch::X86), cfg!(feature = "x86"));
        assert_eq!(supports(Arch::PowerPC), cfg!(feature = "powerpc"));
        assert_eq!(supports(Arch::Sparc), cfg!(feature = "sparc"));
        assert_eq!(supports(Arch::SystemZ), cfg!(feature = "systemz"));
        assert_eq!(supports(Arch::XCore), cfg!(feature = "xcore"));
        assert_eq!(supports(Arch::M68K), cfg!(feature = "m68k"));
        assert_eq!(supports(Arch::Tms320C64X), cfg!(feature = "tms320c64x"));
        assert_eq!(supports(Arch::M680X), cfg!(feature = "m680x"));
        assert_eq!(supports(Arch::Evm), cfg!(feature = "evm"));
        assert_eq!(supports(Arch::Mos65XX), cfg!(feature = "mos65xx"));

        assert_eq!(supports(SupportQuery::Diet), cfg!(feature = "diet"));
        assert_eq!(
            supports(SupportQuery::X86Reduce),
            cfg!(feature = "x86-reduce")
        );
    }
}
