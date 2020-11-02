pub mod binary;
mod dwarf;
pub mod error;
mod pdb;
mod strmatch;
pub mod symbol;

use self::binary::Binary;
use self::error::Error;
use self::symbol::Symbol;
use capstone::Capstone;

pub fn disasm(binary: &Binary, symbol: &Symbol) -> Result<Disassembly, Error> {
    let caps = capstone_for_binary(binary)?;
    let mut disassembly = Disassembly::new();
    disasm_symbol_lines(&caps, binary, symbol, &mut disassembly)?;
    Ok(disassembly)
}

fn disasm_symbol_lines(
    caps: &Capstone,
    binary: &Binary,
    symbol: &Symbol,
    disassembly: &mut Disassembly,
) -> Result<(), Error> {
    for insn in caps.disasm_iter(
        &binary.data()[symbol.offset()..symbol.end()],
        symbol.address(),
    ) {
        let insn =
            insn.map_err(|err| Error::new("failed to disassemble instruction", Box::new(err)))?;

        let line = DisasmLine {
            address: insn.address(),
            mnemonic: insn.mnemonic().into(),
            operands: insn.operands().into(),
            comments: None,
            bytes: insn.bytes().to_vec().into_boxed_slice(),
            source_lines: None,
        };

        disassembly.push_line(line);
    }
    Ok(())
}

/// Creates a Capstone instance for the binary.
fn capstone_for_binary(binary: &Binary) -> Result<Capstone, Error> {
    use binary::Arch as BinArch;
    use capstone::{Arch as CapArch, Mode};

    let capstone_arch = match binary.arch() {
        BinArch::Unknown => return Err(Error::msg("unknown binary architecture")),
        BinArch::X86 => CapArch::X86,
        BinArch::X86_64 => CapArch::X86,
        BinArch::Arm => CapArch::Arm,
        BinArch::AArch64 => CapArch::Arm64,
    };

    let mut mode = Mode::empty();

    match binary.endian() {
        binary::Endian::Little => mode |= Mode::LittleEndian,
        binary::Endian::Big => mode |= Mode::BigEndian,
        #[cfg(target_endian = "little")]
        binary::Endian::Unknown => mode |= Mode::LittleEndian,
        #[cfg(target_endian = "big")]
        binary::Endian::Unknown => mode |= Mode::BigEndian,
    }

    if binary.arch() == BinArch::X86_64 {
        mode |= Mode::Bits64;
    }

    let mut caps = Capstone::open(capstone_arch, mode)
        .map_err(|err| Error::new("failed to initialize capstone", Box::new(err)))?;
    caps.set_details_enabled(true)
        .map_err(|err| Error::new("failed to enable capstone detail mode", Box::new(err)))?;

    Ok(caps)
}

pub struct Disassembly {
    lines: Vec<DisasmLine>,
    jumps: Vec<Option<JumpTarget>>,
}

impl Disassembly {
    fn new() -> Disassembly {
        Disassembly {
            lines: Vec::new(),
            jumps: Vec::new(),
        }
    }

    fn push_line(&mut self, line: DisasmLine) {
        self.jumps.push(None);
        self.lines.push(line)
    }

    pub fn lines(&self) -> &[DisasmLine] {
        &*self.lines
    }
}

pub struct DisasmLine {
    address: u64,
    mnemonic: Box<str>,
    operands: Box<str>,
    comments: Option<Box<str>>,
    bytes: Box<[u8]>,
    source_lines: Option<Box<[Box<str>]>>,
}

impl DisasmLine {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn mnemonic(&self) -> &str {
        &*self.mnemonic
    }

    pub fn operands(&self) -> &str {
        &*self.operands
    }

    pub fn comments(&self) -> &str {
        self.comments.as_deref().unwrap_or("")
    }

    pub fn bytes(&self) -> &[u8] {
        &*self.bytes
    }

    pub fn source_lines(&self) -> &[Box<str>] {
        self.source_lines.as_deref().unwrap_or(&[])
    }
}

pub struct JumpTarget {
    pub index: usize,
    pub address: u64,
}
