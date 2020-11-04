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
    let mut sources = Vec::new();
    let mut sources_auto = false;
    for s in opts.symbol_source.split(',').map(|split| split.trim()) {
        use disasm::symbol::SymbolSource;
        if s.eq_ignore_ascii_case("auto") {
            sources_auto = true;
            break;
        } else if s.eq_ignore_ascii_case("elf") {
            sources.push(SymbolSource::Elf);
        } else if s.eq_ignore_ascii_case("mach") {
            sources.push(SymbolSource::Mach);
        } else if s.eq_ignore_ascii_case("pe") {
            sources.push(SymbolSource::Pe);
        } else if s.eq_ignore_ascii_case("archive") {
            sources.push(SymbolSource::Archive);
        } else if s.eq_ignore_ascii_case("obj") {
            sources.push(SymbolSource::Elf);
            sources.push(SymbolSource::Mach);
            sources.push(SymbolSource::Pe);
            sources.push(SymbolSource::Archive);
        } else if s.eq_ignore_ascii_case("dwarf") {
            sources.push(SymbolSource::Dwarf);
        } else if s.eq_ignore_ascii_case("pdb") {
            sources.push(SymbolSource::Pdb);
        } else if s.eq_ignore_ascii_case("debug") {
            sources.push(SymbolSource::Dwarf);
            sources.push(SymbolSource::Pdb);
        }
    }
    sources_auto |= sources.is_empty();
    sources.sort_unstable();
    sources.dedup();
    let sources = if sources_auto { None } else { Some(sources) };
    let bin = Binary::new(data, sources.as_deref())?;

    if let Some(symbol) = bin.fuzzy_find_symbol(&opts.symbol) {
        let disassembly = disasm::disasm(&bin, symbol)?;

        println!("{}:", symbol.name());
        for line in disassembly.lines() {
            println!(
                "{:8x}  {:8}  {}",
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
