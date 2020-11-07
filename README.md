cargo-disasm
============
[![crates.io version][crate-shield]][crate] [![build status][build-shield]][build-status] [![license][license-shield]][license]

> A cargo subcommand that displays the assembly generated for a function.


> **This is still under heavy development**
>
> For now `cargo-disasm` can disassemble symbols from `ELF` object files
> for binary crates and make use of DWARF debug information for symbol discovery.
> Check [here](#todo) to see the current progress.

```sh
cargo install cargo-disasm
```

# Usage

To view the assembly of a function `foo::bar::baz()`, a function `bar` in submodule
`bar` in crate `foo`, the subcommand can be run from your crate's root directory:
```sh
cargo disasm foo::bar::baz
```

Sometimes `cargo-disasm` has trouble finding your symbols in `release` mode. To make
sure that `cargo-disasm` is searching all sources available, `--symsrc=all` can be
passed as an argument like so:
```sh
cargo disasm --release --symsrc=all foo::bar::baz
```
> This solution is temporary and the default `--symsrc=auto` should
> be able to figure this out on its own soon.

# TODO
- [ ] Optional arrows for displaying jump sources and targets
- [ ] Showing source code alongside disassembly
- [ ] Syntax highlighting for disassembly

**Windows**  
- [ ] PE/COFF file disassembly and symbol discovery
- [ ] use PDB for symbol discovery and line information

**MacOS**  
- [ ] Mach file disassembly and symbol discovery
- [ ] use dSYM (DWARF) for symbol discovery and line information

**Linux**  
- [x] ELF file disassembly and symbol discovery
- [x] use DWARF for symbol discovery and line information

[crate]: https://crates.io/crates/cargo-disasm
[crate-shield]: https://img.shields.io/crates/v/cargo-disasm?style=flat-square
[build-shield]: https://img.shields.io/github/workflow/status/ExPixel/cargo-disasm/Test?style=flat-square
[build-status]: https://github.com/ExPixel/cargo-disasm/actions?query=workflow%3ATest
[license-shield]: https://img.shields.io/github/license/expixel/cargo-disasm?style=flat-square
[license]: https://github.com/ExPixel/cargo-disasm/blob/main/LICENSE.txt
