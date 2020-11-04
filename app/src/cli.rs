use clap::Clap;
use std::path::PathBuf;
use termcolor::ColorChoice;

#[derive(Clap)]
#[clap(name = "cargo-disasm", version = "0.0.1", author = "Marc C.")]
pub struct Opts {
    pub symbol: String,
    pub binary: PathBuf,

    /// Comma separated list of sources that will be used for finding symbols.
    /// By default this is `auto`.
    ///
    /// Possible values are: auto, dwarf, pdb, elf, pe, mach, archive,
    /// obj (elf + pe + mach + archive), debug (dwarf + pdb),
    /// all (use everything)
    #[clap(long = "symsrc", multiple = true, use_delimiter = true)]
    pub symbol_sources: Vec<String>,

    /// Path to Cargo.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<PathBuf>,

    /// Disassemble the release mode build artifacts.
    #[clap(long = "release")]
    pub release: bool,

    /// Sets the log level: 0 - +warning +error, 1 - +info, 2 - +debug, 3 - +trace
    /// The `silent` flag can be used to turn off logging completely.
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: u32,

    /// Disables logging.
    #[clap(long)]
    pub silent: bool,

    /// Coloring: auto, always, never, and always-ansi (only uses ansi color codes).
    #[clap(long = "color", default_value = "auto", parse(try_from_str = parse_colorchoice))]
    pub color_choice: ColorChoice,
}

impl Opts {
    pub fn log_level_filter(&self) -> log::LevelFilter {
        use log::LevelFilter;

        if self.silent {
            return LevelFilter::Off;
        }

        match self.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

pub fn parse_colorchoice(s: &str) -> Result<ColorChoice, String> {
    if s.eq_ignore_ascii_case("auto") {
        Ok(ColorChoice::Auto)
    } else if s.eq_ignore_ascii_case("always") {
        Ok(ColorChoice::Always)
    } else if s.eq_ignore_ascii_case("never") {
        Ok(ColorChoice::Never)
    } else if s.eq_ignore_ascii_case("always-ansi") {
        Ok(ColorChoice::AlwaysAnsi)
    } else {
        Err(format!("{} is not a valid color value", s))
    }
}
