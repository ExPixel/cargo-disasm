mod cli;

use clap::Clap as _;
use cli::{DisasmOpts, Opts, SubOpts};
use disasm::{
    binary::{Binary, BinaryData},
    symbol::Symbol,
};
use std::error::Error;

fn main() {
    let opts = Opts::parse();

    if let Err(err) = run(&opts) {
        eprintln!("error: {}", err);
        let mut last_source: &dyn Error = &*err;
        while let Some(source) = last_source.source() {
            eprintln!("error:   caused by {}", source);
            last_source = source;
        }
    }
}

fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    match opts.subcmd {
        SubOpts::Disasm(ref disasm_opts) => disasm(disasm_opts, opts),
    }
}

fn disasm(opts: &DisasmOpts, _main_opts: &Opts) -> Result<(), Box<dyn Error>> {
    use std::fs::File;

    let file = File::open(&opts.binary)?;
    let data = BinaryData::from_file(&file)?;
    let bin = Binary::new(data.clone())?;

    if let Some(symbol) = bin.fuzzy_find_symbol(&opts.symbol) {
        use capstone::{Arch, Capstone, Mode};

        println!("{}:", symbol.name());

        let caps = Capstone::open(Arch::X86, Mode::LittleEndian)?;
        for insn in caps.disasm_iter(&data[symbol.offset()..symbol.end()], symbol.address()) {
            let insn = insn?;
            println!("  {} {}", insn.mnemonic(), insn.operands());
        }
    } else {
        return Err(format!("no symbol matching `{}` was found", opts.symbol).into());
    }

    Ok(())
}

// #[derive(Eq, PartialEq)]
// pub struct ComparedSymbol {
//     distance: u32,
//     symbol: Symbol,
// }

// impl std::cmp::Ord for ComparedSymbol {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.distance
//             .cmp(&other.distance)
//             .then_with(|| self.symbol.address().cmp(&other.symbol.address()))
//             .then_with(|| self.symbol.offset().cmp(&other.symbol.offset()))
//             .then_with(|| self.symbol.name().cmp(&other.symbol.name()))
//     }
// }

// impl std::cmp::PartialOrd for ComparedSymbol {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }
