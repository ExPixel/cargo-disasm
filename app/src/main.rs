mod cli;
mod logging;

use clap::Clap as _;
use cli::Opts;
use disasm::binary::{Binary, BinaryData};
use logging::AppLogger;
use std::error::Error;
use termcolor::ColorChoice;

fn main() {
    log::set_logger(AppLogger::init()).expect("failed to set logger");
    let has_err = if let Err(err) = run() {
        log::error!("{}", err);
        let mut last_source: &dyn Error = &*err;
        while let Some(source) = last_source.source() {
            log::error!("  caused by {}", source);
            last_source = source;
        }
        true
    } else {
        false
    };
    log::logger().flush();

    if has_err {
        std::process::exit(-1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    use std::fs::File;

    let opts = Opts::parse();

    unsafe { AppLogger::instance().set_level(opts.log_level_filter()) };
    match opts.color_choice {
        ColorChoice::Auto => unsafe {
            AppLogger::instance().set_color_choice_out(if atty::is(atty::Stream::Stdout) {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            });

            AppLogger::instance().set_color_choice_err(if atty::is(atty::Stream::Stderr) {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            });
        },

        choice => unsafe {
            AppLogger::instance().set_color_choice_out(choice);
            AppLogger::instance().set_color_choice_err(choice);
        },
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
                "{:8x}  {:>8}  {:32}",
                line.address(),
                line.mnemonic(),
                line.operands(),
            );
        }
    // disasm::print_disassembly(&disassembly, ||);
    } else {
        return Err(format!("no symbol matching `{}` was found", opts.symbol).into());
    }

    Ok(())
}
