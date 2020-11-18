cargo-disasm
============
[![crates.io version][crate-shield]][crate] [![build status][build-shield]][build-status] [![license][license-shield]][license]

> A cargo subcommand that displays the assembly generated for a function.
> `cargo-disasm` does not require recompiling your project, it disassembles
> and finds symbols in your binary directly.


> **This is still under heavy development**
>
> For now `cargo-disasm` can disassemble symbols from `ELF` *(Linux)*, `Mach` *(MacOS)*, an `PE/COFF` *(Windows)* object files
> for binary crates and make use of DWARF and PDB debug information for symbol discovery.
> Check [here](#todo) to see the current progress.

```sh
cargo install cargo-disasm
```

[![asciicast demo](https://asciinema.org/a/371231.svg)](https://asciinema.org/a/371231)

# Usage

To view the assembly of a function `foo::bar::baz()`, a function `baz` in module
`bar` in crate `foo`, the subcommand can be run from your crate's root directory:
```sh
# Make sure that your project has a binary to disassemble first:
cargo build
cargo disasm foo::bar::baz
```

Sometimes `cargo-disasm` has trouble finding your symbols in `release` mode. To make
sure that `cargo-disasm` is searching all sources available, `--symsrc=all` can be
passed as an argument like so:
```sh
# Make sure that your project has a release binary to disassemble first:
cargo build --release
cargo disasm --release --symsrc=all foo::bar::baz
```
> This solution is temporary and the default `--symsrc=auto` should
> be able to figure this out on its own soon.

# TODO
- [ ] Showing source code alongside disassembly

**Windows**  
- [x] PE/COFF file disassembly and symbol discovery
- [x] use PDB for symbol discovery *(MSVC toolchain)*
- [ ] use DWARF for symbol discovery *(GNU)*

**MacOS**  
- [x] Mach file disassembly and symbol discovery
- [x] use dSYM (DWARF) for symbol discovery

**Linux**  
- [x] ELF file disassembly and symbol discovery
- [x] use DWARF for symbol discovery

**Line Information**
- [ ] use DWARF for line information
- [ ] use PDB for line information

**Postponed**
- ~~Syntax highlighting for disassembly~~ (good for higher level source code, unecessary for assembly)
- ~~Optional arrows for displaying jump sources and targets~~ (too noisy)

[crate]: https://crates.io/crates/cargo-disasm
[crate-shield]: https://img.shields.io/crates/v/cargo-disasm?style=flat-square
[build-shield]: https://img.shields.io/github/workflow/status/ExPixel/cargo-disasm/Test?style=flat-square
[build-status]: https://github.com/ExPixel/cargo-disasm/actions?query=workflow%3ATest
[license-shield]: https://img.shields.io/github/license/expixel/cargo-disasm?style=flat-square
[license]: https://github.com/ExPixel/cargo-disasm/blob/main/LICENSE.txt
