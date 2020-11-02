mod cli;

use clap::Clap as _;
use cli::{DisasmOpts, Opts, SubOpts};
use disasm::binary::{Binary, BinaryData};
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
    let bin = Binary::new(data)?;

    if let Some(symbol) = bin.fuzzy_find_symbol(&opts.symbol) {
        let disassembly = disasm::disasm(&bin, symbol)?;

        println!("{}:", symbol.name());
        for line in disassembly.lines() {
            println!(
                "{:8x}    {:8}  {}",
                line.address(),
                line.mnemonic(),
                line.operands()
            );
        }
    // disasm::print_disassembly(&disassembly, ||);
    } else {
        return Err(format!("no symbol matching `{}` was found", opts.symbol).into());
    }

    Ok(())
}
