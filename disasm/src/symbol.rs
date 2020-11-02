use std::borrow::Cow;
use std::fmt;

#[derive(Eq, PartialEq)]
pub struct Symbol {
    /// The demangled name of the symbol.
    name: String,

    /// The virtual address of the symbol.
    addr: u64,

    /// The starting byte position of the symbol in its binary.
    bpos: usize,

    /// The length of the symbol in its binary.
    blen: usize,

    /// Possible source language of the symbol.
    lang: SymbolLang,

    /// Where this symbol is from.
    source: SymbolSource,

    /// The type of this symbol.
    type_: SymbolType,
}

impl Symbol {
    pub fn new<'a, N>(
        name: N,
        addr: u64,
        bpos: usize,
        blen: usize,
        type_: SymbolType,
        source: SymbolSource,
        mut lang: SymbolLang,
    ) -> Self
    where
        N: Into<Cow<'a, str>>,
    {
        use cpp_demangle::Symbol as CppSymbol;
        use rustc_demangle::try_demangle;

        // FIXME demangle C names (e.g. stdcall and fastcall naming conventions).
        let name = name.into();
        let demangled_name = try_demangle(&*name)
            .map(|n| {
                lang.update(SymbolLang::Rust);
                Cow::from(format!("{}", n))
            })
            .or_else(|_| {
                CppSymbol::new(name.as_bytes()).map(|s| {
                    lang.update(SymbolLang::Cpp);
                    Cow::from(s.to_string())
                })
            })
            .unwrap_or_else(|_| name);

        Symbol {
            name: demangled_name.into_owned(),
            addr,
            bpos,
            blen,
            type_,
            source,
            lang,
        }
    }

    pub fn address(&self) -> u64 {
        self.addr
    }

    pub fn end_address(&self) -> u64 {
        self.addr + (self.blen as u64)
    }

    pub fn offset(&self) -> usize {
        self.bpos
    }

    pub fn end(&self) -> usize {
        self.bpos + self.blen
    }

    pub fn size(&self) -> usize {
        self.blen
    }

    pub fn name(&self) -> &str {
        &*self.name
    }

    pub fn lang(&self) -> SymbolLang {
        self.lang
    }

    pub fn source(&self) -> SymbolSource {
        self.source
    }

    pub fn type_(&self) -> SymbolType {
        self.type_
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SymbolType {
    Function,

    /// Static variable.
    Static,
}

impl fmt::Display for SymbolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            SymbolType::Function => "function",
            SymbolType::Static => "static",
        };
        write!(f, "{}", t)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum SymbolLang {
    Rust,
    Cpp,
    C,
    Unknown,
}

impl SymbolLang {
    /// Update the language if it is unknown.
    fn update(&mut self, new_lang: SymbolLang) {
        if *self == SymbolLang::Unknown {
            *self = new_lang
        }
    }
}

impl fmt::Display for SymbolLang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            SymbolLang::Rust => "Rust",
            SymbolLang::Cpp => "C++",
            SymbolLang::C => "C",
            SymbolLang::Unknown => "unknown",
        };
        write!(f, "{}", t)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolSource {
    /// The symbol was stored as part of the object file's (elf, mach-o, archive, pe, ...)
    /// structure.
    Object = 0,

    /// The symbol was stored in DWARF debug data.
    Dwarf = 1,

    /// The symbol was found in a PDB.
    PDB = 2,
}

impl std::cmp::Ord for SymbolSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // object > dwarf > PDB
        // ^ this means that PDB symbols have HIGHER priority than DWARF
        // and DWARF has HIGHER priority than object file symbols.
        (*self as u8).cmp(&(*other as u8)).reverse()
    }
}

impl std::cmp::PartialOrd for SymbolSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SymbolSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            SymbolSource::Object => "object",
            SymbolSource::Dwarf => "DWARF",
            SymbolSource::PDB => "PDB",
        };
        write!(f, "{}", t)
    }
}
