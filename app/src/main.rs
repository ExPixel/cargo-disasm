mod cli;
mod logging;

use clap::Clap as _;
use cli::Opts;
use disasm::binary::{Binary, BinaryData};
use log::LevelFilter;
use logging::AppLogger;
use std::error::Error;
use termcolor::ColorChoice;

fn main() {
    log::set_logger(AppLogger::init()).expect("failed to set logger");

    let opts = Opts::parse();

    if let Err(err) = run(&opts) {
        log::error!("{}", err);
        let mut last_source: &dyn Error = &*err;
        while let Some(source) = last_source.source() {
            log::error!("  caused by {}", source);
            last_source = source;
        }
    }

    log::logger().flush();
}

fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    use std::fs::File;

    if let Some(level) = opts.log {
        unsafe { AppLogger::instance().set_level(level) };
    }

    let file = File::open(&opts.binary)?;
    let data = BinaryData::from_file(&file)?;
    let mut sources = Vec::new();
    let mut sources_auto = false;
    for s in opts.symbol_sources.iter() {
        use disasm::symbol::SymbolSource;
        if s.eq_ignore_ascii_case("all") {
            // object file formats
            sources.push(SymbolSource::Elf);
            sources.push(SymbolSource::Mach);
            sources.push(SymbolSource::Pe);
            sources.push(SymbolSource::Archive);

            // debug formats
            sources.push(SymbolSource::Dwarf);
            sources.push(SymbolSource::Pdb);

            break;
        } else if s.eq_ignore_ascii_case("auto") {
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
        } else {
            return Err(format!("{} is not a valid symbol source", s).into());
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
