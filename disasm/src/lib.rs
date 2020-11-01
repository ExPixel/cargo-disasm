pub mod binary;
pub mod error;
mod strmatch;
pub mod symbol;

use self::binary::Binary;
use self::error::Error;
use self::symbol::Symbol;

pub fn disasm(binary: &Binary, symbol: &Symbol) -> Result<Disassembly, Error> {
    todo!();
}

pub struct Disassembly {
    lines: Vec<DisasmLine>,
    jumps: Vec<Option<JumpTarget>>,
}

pub struct DisasmLine {
    mnemonic: String,
    operands: String,
    comments: String,
    opcode: Vec<u8>,
}

pub struct JumpTarget {
    pub index: usize,
    pub address: u64,
}
