mod cli;
mod logging;

use clap::Clap as _;
use cli::Opts;
use disasm::binary::{Binary, BinaryData};
use logging::AppLogger;
use std::error::Error;
use std::path::{Path, PathBuf};
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

    let binary_path = find_binary_path(&opts)?;
    log::debug!("using binary {}", binary_path.display());
    let file = File::open(binary_path)?;
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

    // FIXME temporary test code
    if let Some(symbol) = bin.fuzzy_find_symbol(&opts.symbol) {
        let disassembly = disasm::disasm(&bin, symbol)?;
        let measure = disasm::display::measure(&disassembly);
        let max_address_width = measure.max_address_width_hex();

        println!("{}:", symbol.name());
        for line in disassembly.lines() {
            println!(
                "  {address:<max_address_width$x}    {mnemonic:<max_mnemonic_len$}  {operands:<max_operands_len$}    {comment_hash}{comments}",
                address = line.address(),
                mnemonic = line.mnemonic(),
                operands = line.operands(),
                comments = line.comments(),
                comment_hash = if line.comments().is_empty() { "" } else { "# " },
                max_mnemonic_len = measure.max_mnemonic_len(),
                max_operands_len = measure.max_operands_len(),
                max_address_width = max_address_width,
            );
        }
    // disasm::print_disassembly(&disassembly, ||);
    } else {
        return Err(format!("no symbol matching `{}` was found", opts.symbol).into());
    }

    Ok(())
}

/// Use options to find the binary to search for the symbol in.
fn find_binary_path(opts: &Opts) -> Result<PathBuf, Box<dyn Error>> {
    use cargo_metadata::{MetadataCommand, Package, Target};
    if let Some(ref b) = opts.binary_path {
        return Ok(b.clone());
    }

    log::trace!("running cargo_metadata");
    let mut cmd = MetadataCommand::new();
    if let Some(ref m) = opts.manifest_path {
        cmd.manifest_path(m);
    }
    let metadata = cmd.exec()?;

    let match_package = |package: &Package| {
        if !metadata.workspace_members.contains(&package.id) {
            return false;
        }

        if let Some(ref p) = opts.package {
            // FIXME use the pkgid scheme instead
            package.name.eq_ignore_ascii_case(p)
        } else {
            true
        }
    };

    let match_target = |target: &Target| {
        if let Some(ref t) = opts.target_name {
            if !target.name.eq_ignore_ascii_case(t) {
                return false;
            }
        }
        // FIXME support the other target types at some point (test, example, bench, lib)
        target.kind.iter().any(|k| k == "bin")
    };

    let found_targets = metadata
        .packages
        .iter()
        .filter(|p| match_package(p))
        .flat_map(|p| p.targets.iter().map(move |t| (p, t)))
        .filter(|(_, t)| match_target(t))
        .collect::<Vec<(&Package, &Target)>>();

    if found_targets.is_empty() {
        return Err("no matching targets were found".into());
    }
    if found_targets.len() > 1 {
        let mut s = String::from("multiple matching targets were found:");
        for (package, target) in found_targets {
            s.push_str(&format!(
                "\n    - `{}` ({}) in package `{}`",
                target.name,
                target.kind.join(", "),
                package.name
            ));
        }
        return Err(s.into());
    }

    let (_package, target) = found_targets.into_iter().next().unwrap();
    let mut path = metadata.target_directory.clone();
    if opts.release {
        path.push("release");
    } else {
        path.push("debug");
    }
    path.push(&target.name);

    #[cfg(target_os = "windows")]
    if !path.is_file() {
        path.pop();
        path.push(format!("{}.exe", target.name));
    }

    Ok(path)
}
