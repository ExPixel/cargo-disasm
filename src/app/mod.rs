pub mod cli;
pub mod logging;
mod printer;

use crate::disasm::{
    self,
    binary::{Binary, BinaryData, SearchOptions},
    symbol::SymbolSource,
};
use anyhow::Context as _;
use clap::Clap as _;
use cli::Opts;
use logging::AppLogger;
use std::path::PathBuf;
use termcolor::ColorChoice;
use termcolor::StandardStream;

fn parse_options() -> Opts {
    if std::env::var("CARGO").is_ok() {
        let mut args = std::env::args_os().collect::<Vec<_>>();
        if args.len() > 2 && args[1] == "disasm" {
            log::trace!("this is being run as a cargo subcommand");
            args.remove(1); // this is just our subcommand forwarded from cargo
            Opts::parse_from(args)
        } else {
            Opts::parse_from(args)
        }
    } else {
        Opts::parse()
    }
}

pub fn run() -> anyhow::Result<()> {
    let opts = parse_options();

    unsafe { AppLogger::instance().set_level(opts.log_level_filter()) };
    let color_choice = match opts.color_choice {
        ColorChoice::Auto => unsafe {
            let out_choice = if atty::is(atty::Stream::Stdout) {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            };
            AppLogger::instance().set_color_choice_out(out_choice);

            AppLogger::instance().set_color_choice_err(if atty::is(atty::Stream::Stderr) {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            });

            out_choice
        },

        choice => unsafe {
            AppLogger::instance().set_color_choice_out(choice);
            AppLogger::instance().set_color_choice_err(choice);
            choice
        },
    };

    let binary_path = find_binary_path(&opts)?;
    log::debug!("using binary {}", binary_path.display());
    let data = BinaryData::from_path(&binary_path)
        .with_context(|| format!("failed to load binary `{}`", binary_path.display()))?;
    let mut sources = Vec::new();
    let mut sources_auto = false;
    for s in opts.symbol_sources.iter() {
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
            return Err(anyhow::anyhow!("{} is not a valid symbol source", s));
        }
    }
    sources_auto |= sources.is_empty();
    sources.sort_unstable();
    sources.dedup();

    let mut search_options = SearchOptions { sources: &sources };
    let bin = Binary::new(data, search_options)?;

    // FIXME temporary test code
    if let Some(symbol) = bin.fuzzy_find_symbol(&opts.symbol) {
        let disassembly = disasm::disasm(&bin, symbol)?;
        let mut stdout = StandardStream::stdout(color_choice);
        printer::print_disassembly(&mut stdout, symbol, &disassembly)
            .context("error occured while printing disassembly")?;
    } else {
        return Err(anyhow::anyhow!(
            "no symbol matching `{}` was found",
            opts.symbol
        ));
    }

    Ok(())
}

/// Use options to find the binary to search for the symbol in.
fn find_binary_path(opts: &Opts) -> anyhow::Result<PathBuf> {
    use cargo_metadata::{MetadataCommand, Package, Target};
    if let Some(ref b) = opts.binary_path {
        return Ok(b.clone());
    }

    log::trace!("running cargo_metadata");
    let mut cmd = MetadataCommand::new();
    if let Some(ref m) = opts.manifest_path {
        cmd.manifest_path(m);
    }
    let metadata = cmd
        .exec()
        .context("error occurred while running cargo_metadata")?;

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
        return Err(anyhow::anyhow!("no matching targets were found"));
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
        return Err(anyhow::anyhow!(s));
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
