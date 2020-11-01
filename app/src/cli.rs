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
}
