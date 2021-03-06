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
    /// Where this symbol is from.
    source: SymbolSource,
}

impl Symbol {
    pub fn new_unmangled(
        name: String,
        addr: u64,
        bpos: usize,
        blen: usize,
        source: SymbolSource,
    ) -> Self {
        Symbol {
            name,
            addr,
            bpos,
            blen,
            source,
        }
    }

    pub fn new<'a, N>(name: N, addr: u64, bpos: usize, blen: usize, source: SymbolSource) -> Self
    where
        N: Into<Cow<'a, str>>,
    {
        use cpp_demangle::Symbol as CppSymbol;
        use rustc_demangle::try_demangle;

        // FIXME demangle C names (e.g. stdcall and fastcall naming conventions).
        let name = name.into();
        let demangled_name = try_demangle(&*name)
            .map(|n| Cow::from(format!("{:#}", n)))
            .or_else(|_| CppSymbol::new(name.as_bytes()).map(|s| Cow::from(s.to_string())))
            .unwrap_or(name);

        Symbol {
            name: demangled_name.into_owned(),
            addr,
            bpos,
            blen,
            source,
        }
    }

    pub fn address(&self) -> u64 {
        self.addr
    }

    /// One byte beyond the end of the symbol.
    pub fn end_address(&self) -> u64 {
        self.addr + (self.blen as u64)
    }

    pub fn address_range(&self) -> std::ops::Range<u64> {
        self.address()..self.end_address()
    }

    pub fn offset(&self) -> usize {
        self.bpos
    }

    pub fn end(&self) -> usize {
        self.bpos + self.blen
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.blen
    }

    pub fn name(&self) -> &str {
        &*self.name
    }

    pub fn source(&self) -> SymbolSource {
        self.source
    }

    pub(crate) fn set_address(&mut self, new_address: u64) {
        self.addr = new_address;
    }

    pub(crate) fn set_size(&mut self, new_size: usize) {
        self.blen = new_size;
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolSource {
    Elf,
    Mach,
    Pe,
    Archive,
    Dwarf,
    Pdb,
}

impl SymbolSource {
    pub fn priority(self) -> u8 {
        match self {
            SymbolSource::Dwarf | SymbolSource::Pdb => 1,
            SymbolSource::Elf | SymbolSource::Mach | SymbolSource::Pe | SymbolSource::Archive => 2,
        }
    }
}

impl std::str::FromStr for SymbolSource {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("elf") {
            Ok(SymbolSource::Elf)
        } else if s.eq_ignore_ascii_case("mach") {
            Ok(SymbolSource::Mach)
        } else if s.eq_ignore_ascii_case("pe") {
            Ok(SymbolSource::Pe)
        } else if s.eq_ignore_ascii_case("archive") {
            Ok(SymbolSource::Archive)
        } else if s.eq_ignore_ascii_case("dwarf") {
            Ok(SymbolSource::Dwarf)
        } else if s.eq_ignore_ascii_case("pdb") {
            Ok(SymbolSource::Pdb)
        } else {
            Err("invalid symbol source")
        }
    }
}

impl std::cmp::Ord for SymbolSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
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
            SymbolSource::Elf => "elf",
            SymbolSource::Mach => "mach",
            SymbolSource::Pe => "pe",
            SymbolSource::Archive => "archive",
            SymbolSource::Dwarf => "dwarf",
            SymbolSource::Pdb => "pdb",
        };
        write!(f, "{}", t)
    }
}
