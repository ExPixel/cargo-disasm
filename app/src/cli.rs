use clap::Clap;
use std::path::PathBuf;

#[derive(Clap)]
#[clap(name = "cargo-disasm", version = "0.0.1", author = "Marc C.")]
pub struct Opts {
    #[clap(subcommand)]
    pub subcmd: SubOpts,
}

#[derive(Clap)]
pub enum SubOpts {
    Disasm(DisasmOpts),
}

#[derive(Clap)]
pub struct DisasmOpts {
    pub symbol: String,
    pub binary: PathBuf,

    /// Comma separated list of sources that will be used for finding symbols.
    /// By default this is `auto`.
    ///
    /// Possible values are: auto, dwarf, pdb, elf, pe, mach, archive,
    /// obj (elf + pe + mach + archive), debug (dwarf + pdb)
    #[clap(long = "symsrc")]
    pub symbol_source: String,
}
