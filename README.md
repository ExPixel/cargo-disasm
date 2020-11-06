# cargo-disasm
Disassembly viewer for rust projects.

** Still very much a work in progress. **

At the moment the application only works on Linux and can be used like this:
```sh
# Create a debug build for more symbol information.
$ cargo build

# Create a release build for faster disassembly.
$ cargo build --release

# Disassemble app::main with verbose output.
$ ./target/release/cargo-disasm -vvv app::main

# Disassemble app::main in release mode with verbose output.
# The incantation is a bit longer here because release mode
# still contains some DWARF debug information but with much
# fewer symbols. `--symsrc=all` will make cargo-disasm search
# both DWARF and ELF symbols.
$ ./target/release/cargo-disasm -vvv --release --symsrc=all app::main
```

Output should look like this:
```
$ ./target/release/app -vvv  char::len_utf8
  trace(app): running cargo_metadata
  debug(app): using binary /home/<me>/code/cargo-disasm/target/debug/app
  debug(disasm::binary): object type   = ELF
  debug(disasm::binary): object bits   = 64-bits
  debug(disasm::binary): object endian = little-endian
  debug(disasm::binary): object arch   = x86_64
  debug(disasm::binary): retrieving symbols from DWARF debug information
  trace(disasm::binary): found 22949 symbols in DWARF debug information in 183.582 ms
  debug(disasm::binary): found 22949 total symbols in 183.729 ms
  trace(disasm::binary): sorted 22949 symbols in 3.079 ms
  trace(disasm::binary): fuzzy matched `char::len_utf8` in 3.252 ms
core::char::methods::len_utf8:
  c4120    sub  rsp, 0x18                             
  c4124    mov  dword ptr [rsp + 0x14], edi           
  c4128    cmp  edi, 0x80                             
  c412e    mov  dword ptr [rsp + 4], edi              
  c4132    jb   core::char::methods::len_utf8+0x21    # 0x803137
  c4134    mov  eax, dword ptr [rsp + 4]              
  c4138    cmp  eax, 0x800                            
  c413d    jb   core::char::methods::len_utf8+0x39    # 0x803161
  c413f    jmp  core::char::methods::len_utf8+0x2c    # 0x803148
  c4141    mov  qword ptr [rsp + 8], 1                
  c414a    jmp  core::char::methods::len_utf8+0x5c    # 0x803196
  c414c    mov  eax, dword ptr [rsp + 4]              
  c4150    cmp  eax, 0x10000                          
  c4155    jb   core::char::methods::len_utf8+0x4f    # 0x803183
  c4157    jmp  core::char::methods::len_utf8+0x44    # 0x803172
  c4159    mov  qword ptr [rsp + 8], 2                
  c4162    jmp  core::char::methods::len_utf8+0x5a    # 0x803194
  c4164    mov  qword ptr [rsp + 8], 4                
  c416d    jmp  core::char::methods::len_utf8+0x58    # 0x803192
  c416f    mov  qword ptr [rsp + 8], 3                
  c4178    jmp  core::char::methods::len_utf8+0x5a    # 0x803194
  c417a    jmp  core::char::methods::len_utf8+0x5c    # 0x803196
  c417c    mov  rax, qword ptr [rsp + 8]              
  c4181    add  rsp, 0x18                             
  c4185    ret             
```
