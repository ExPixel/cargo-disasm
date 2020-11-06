# cargo-disasm
Disassembly viewer for rust projects.

** Still very much a work in progress. **

At the moment the application only works on Linux and can be used like this:
```sh
# create a debug build for more symbol information
$ cargo build

# create a release build for faster disassembly
$ cargo build --release

# disassemble app::main with verbose output
$ ./target/release/app -vvv app::main ./target/debug/app
```
