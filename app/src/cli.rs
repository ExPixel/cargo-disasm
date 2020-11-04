use clap::Clap;
use std::path::PathBuf;

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

    /// Sets the log level. Possible values are (error, warn, info, debug, trace).
    #[clap(long = "log")]
    pub log: Option<log::LevelFilter>,
}
