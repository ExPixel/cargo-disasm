pub mod binary;
pub mod display;
pub mod source;
pub mod symbol;

mod anal;
mod dwarf;
mod pdb;
pub mod strmatch;

pub use self::anal::Jump;
use self::binary::Binary;
use self::symbol::Symbol;
use anyhow::Context as _;
use capstone::Capstone;
use source::SourceLoader;

pub fn disasm(binary: &Binary, symbol: &Symbol, load_source: bool) -> anyhow::Result<Disassembly> {
    let disasm_timer = std::time::Instant::now();
    let caps = capstone_for_binary(binary)?;
    let mut disassembly = Disassembly::new();
    let source_loader = if load_source {
        Some(SourceLoader::new())
    } else {
        None
    };
    disasm_symbol_lines(&caps, binary, symbol, source_loader, &mut disassembly)?;
    log::trace!(
        "disassembled symbol {} in {}",
        symbol.name(),
        crate::util::DurationDisplay(disasm_timer.elapsed())
    );
    Ok(disassembly)
}

fn disasm_symbol_lines(
    caps: &Capstone,
    binary: &Binary,
    symbol: &Symbol,
    mut source_loader: Option<SourceLoader>,
    disassembly: &mut Disassembly,
) -> anyhow::Result<()> {
    for insn in caps.disasm_iter(
        &binary.data()[symbol.offset()..symbol.end()],
        symbol.address(),
    ) {
        let insn = insn.context("failed to disassemble instruction")?;
        let jump = anal::identify_jump_target(insn, caps);

        let mut source_lines = Vec::new();
        if let Some(ref mut source_loader) = source_loader {
            source_loader
                .load_lines(
                    binary.addr2line(insn.address())?.iter_mut().flatten(),
                    &mut source_lines,
                )
                .context("error while loading sources for line")?;
        }
        let source_lines = if source_lines.is_empty() {
            None
        } else {
            Some(source_lines.into_boxed_slice())
        };

        let line = DisasmLine {
            address: insn.address(),
            mnemonic: insn.mnemonic().into(),
            operands: insn.operands().into(),
            comments: None,
            bytes: insn.bytes().to_vec().into_boxed_slice(),
            source_lines,
            jump,
            is_symbolicated_jump: false,
        };
        disassembly.push_line(line);
    }
    symbolicate_and_internalize_jumps(binary, symbol, disassembly);
    Ok(())
}

fn symbolicate_and_internalize_jumps(
    binary: &Binary,
    symbol: &Symbol,
    disassembly: &mut Disassembly,
) {
    for idx in 0..disassembly.lines.len() {
        let jump_addr = if let Jump::External(addr) = disassembly.lines[idx].jump {
            addr
        } else {
            continue;
        };

        // This is an internal jump, so we can skip the more
        // expensive symbolication step.
        if symbol.address_range().contains(&jump_addr) {
            disassembly.lines[idx].operands =
                format!("{}+0x{:x}", symbol.name(), jump_addr - symbol.address()).into();
            disassembly.lines[idx].comments = Some(format!("0x{:x}", jump_addr).into());
            disassembly.lines[idx].is_symbolicated_jump = true;

            if let Some(index) = disassembly
                .lines
                .iter()
                .position(|l| l.contains_addr(jump_addr))
            {
                disassembly.lines[idx].jump = Jump::Internal(index);
            }
        } else if let Some((symbol, offset)) = binary.symbolicate(jump_addr) {
            if offset == 0 {
                disassembly.lines[idx].operands = symbol.name().into();
            } else {
                disassembly.lines[idx].operands =
                    format!("{}+0x{:x}", symbol.name(), offset).into();
            }
            disassembly.lines[idx].comments = Some(format!("0x{:x}", jump_addr).into());
            disassembly.lines[idx].is_symbolicated_jump = true;
        }
    }
}

/// Creates a Capstone instance for the binary.
fn capstone_for_binary(binary: &Binary) -> anyhow::Result<Capstone> {
    use binary::Arch as BinArch;
    use capstone::{Arch as CapArch, Mode};

    let capstone_arch = match binary.arch() {
        BinArch::Unknown => {
            return Err(anyhow::anyhow!(
                "unknown or unsupported binary architecture"
            ))
        }
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

    let mut caps = Capstone::open(capstone_arch, mode).context("failed to initialize Capstone")?;
    caps.set_details_enabled(true)
        .context("failed to enable Capstone detail mode")?;

    Ok(caps)
}

pub struct Disassembly {
    lines: Vec<DisasmLine>,
}

impl Disassembly {
    fn new() -> Disassembly {
        Disassembly { lines: Vec::new() }
    }

    fn push_line(&mut self, line: DisasmLine) {
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
    jump: Jump,
    is_symbolicated_jump: bool,
}

impl DisasmLine {
    pub fn contains_addr(&self, addr: u64) -> bool {
        addr >= self.address && addr < self.address + (self.bytes.len() as u64)
    }

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

    pub fn jump(&self) -> Jump {
        self.jump
    }

    pub fn is_symbolicated_jump(&self) -> bool {
        self.is_symbolicated_jump
    }
}
