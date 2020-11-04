use crate::dwarf::DwarfInfo;
use crate::error::Error;
use crate::pdb::PDBInfo;
use crate::strmatch::{distance, Tokenizer};
use crate::symbol::{Symbol, SymbolLang, SymbolSource, SymbolType};
use goblin::{archive::Archive, elf::Elf, mach::Mach, pe::PE, Object};
use memmap::{Mmap, MmapOptions};
use std::convert::TryFrom as _;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::rc::Rc;

pub struct Binary {
    /// Shared binary data. This must be pinned because it is referred to
    data: BinaryData,

    /// DWARF debugging information that was found.
    dwarf: Option<Box<DwarfInfo>>,

    /// PDB debugging information that was found.
    pdb: Option<Box<PDBInfo>>,

    arch: Arch,
    endian: Endian,
    bits: Bits,

    /// A vector of symbols that are sorted by their address in ascending order.
    symbols: Vec<Symbol>,
}

impl Binary {
    pub fn new(data: BinaryData, sources: Option<&[SymbolSource]>) -> Result<Binary, Error> {
        let mut binary = Binary {
            data,
            dwarf: None,
            pdb: None,

            arch: Arch::Unknown,
            endian: Endian::Unknown,
            bits: Bits::Unknown,

            symbols: Vec::new(),
        };

        binary.symbols.sort_unstable_by(|lhs, rhs| {
            lhs.address()
                .cmp(&rhs.address())
                .then(lhs.end_address().cmp(&rhs.end_address()))
        });

        binary.parse_object(sources).map(|_| binary)
    }

    /// Returns an iterator of symbols matching the given `name` string
    /// and their calculated "distance" from the desired symbol name.
    pub fn fuzzy_list_symbols<'s, 'n: 's>(
        &'s self,
        name: &'n str,
    ) -> impl Iterator<Item = (u32, &'s Symbol)> + 's {
        let tokens = Tokenizer::new(name).collect::<Vec<&str>>();
        self.symbols.iter().filter_map(move |sym| {
            Some((
                distance(tokens.iter().copied(), Tokenizer::new(&sym.name()))?,
                sym,
            ))
        })
    }

    pub fn fuzzy_find_symbol<'s>(&'s self, name: &str) -> Option<&'s Symbol> {
        let tokens = Tokenizer::new(name).collect::<Vec<&str>>();
        self.symbols
            .iter()
            .filter_map(|sym| {
                Some((
                    distance(tokens.iter().copied(), Tokenizer::new(&sym.name()))?,
                    sym,
                ))
            })
            .min_by(|lhs, rhs| {
                lhs.0
                    .cmp(&rhs.0)
                    .then_with(|| lhs.1.source().cmp(&rhs.1.source()))
                    .then_with(|| lhs.1.address().cmp(&rhs.1.address()))
                    .then_with(|| lhs.1.offset().cmp(&rhs.1.offset()))
                    .then_with(|| lhs.1.name().cmp(&rhs.1.name()))
            })
            .map(|(_, sym)| sym)
    }

    pub fn data(&self) -> &[u8] {
        &*self.data
    }

    pub fn arch(&self) -> Arch {
        self.arch
    }

    pub fn endian(&self) -> Endian {
        self.endian
    }

    pub fn bits(&self) -> Bits {
        self.bits
    }

    fn parse_object(&mut self, sources: Option<&[SymbolSource]>) -> Result<(), Error> {
        let data = self.data.clone();
        match Object::parse(&data)
            .map_err(|err| Error::new("failed to parse object", Box::new(err)))?
        {
            Object::Elf(elf) => self.parse_elf_object(&elf, sources),
            Object::PE(pe) => self.parse_pe_object(&pe),
            Object::Mach(mach) => self.parse_mach_object(&mach),
            Object::Archive(archive) => self.parse_archive_object(&archive),
            Object::Unknown(magic) => Err(Error::msg(format!(
                "failed to parse object with magic value 0x{:X}",
                magic
            ))),
        }
    }

    fn parse_elf_object(
        &mut self,
        elf: &Elf,
        sources: Option<&[SymbolSource]>,
    ) -> Result<(), Error> {
        use goblin::elf::header;

        self.bits = Bits::from_elf_class(elf.header.e_ident[header::EI_CLASS]);
        self.endian = Endian::from(
            elf.header
                .endianness()
                .map_err(|e| Error::new("failed to identify ELF endianness", Box::new(e)))?,
        );
        self.arch = Arch::from_elf_machine(elf.header.e_machine);

        let mut load_elf_symbols = false;
        let mut load_dwarf_symbols = sources.is_none(); // `auto` makes this true
        for &source in sources.iter().copied().flatten() {
            match source {
                SymbolSource::Elf => load_elf_symbols = true,
                SymbolSource::Dwarf => load_dwarf_symbols = true,
                _ => {}
            }
        }

        let has_dwarf_debug_info = elf
            .section_headers
            .iter()
            .filter_map(|header| {
                // ugh
                elf.shdr_strtab
                    .get(header.sh_name)
                    .transpose()
                    .ok()
                    .flatten()
            })
            .any(|name| DWARF_SECTIONS.contains(&name));

        if has_dwarf_debug_info {
            use gimli::EndianReader;
            use gimli::RunTimeEndian;

            let endian = RunTimeEndian::from(self.endian);

            let loader = |section: gimli::SectionId| {
                self.get_elf_section_data_by_name(&elf, section.name())
                    .map(|d| EndianReader::new(d, endian))
            };
            let sup_loader =
                |_section: gimli::SectionId| Ok(EndianReader::new(self.data.slice(0..0), endian));
            let dwarf = Box::new(DwarfInfo::new(loader, sup_loader)?);

            if load_dwarf_symbols {
                let mut sections: Vec<(std::ops::Range<u64>, usize)> = elf
                    .section_headers
                    .iter()
                    .filter(|header| header.sh_addr != 0) // does not appear in the process memory
                    .map(|header| {
                        (
                            header.sh_addr..(header.sh_addr + header.sh_size),
                            header.sh_offset as usize,
                        )
                    })
                    .collect();
                sections.sort_unstable_by(|(lhs, _), (rhs, _)| {
                    lhs.start.cmp(&rhs.start).then(lhs.end.cmp(&rhs.end))
                });

                let addr_to_offset = |addr| {
                    sections
                        .binary_search_by(|(probe, _)| {
                            if probe.start > addr {
                                std::cmp::Ordering::Greater
                            } else if probe.end <= addr {
                                std::cmp::Ordering::Less
                            } else {
                                std::cmp::Ordering::Equal
                            }
                        })
                        .ok()
                        .map(|idx| {
                            let &(ref range, off) = &sections[idx];
                            (addr - range.start) as usize + off
                        })
                };
                dwarf.load_symbols(&mut self.symbols, addr_to_offset)?;
            }
            self.dwarf = Some(dwarf);
        }

        // If we're using `auto` for the symbol source and no symbols are found.
        if sources.is_none() && self.symbols.is_empty() {
            load_elf_symbols = true;
        }

        if load_elf_symbols {
            self.gather_elf_symbols(elf)?;
        }

        Ok(())
    }

    fn gather_elf_symbols(&mut self, elf: &Elf) -> Result<(), Error> {
        for sym in elf.syms.iter().filter(|sym| sym.is_function()) {
            // FIXME handle symbols with a size of 0 (usually external symbols).
            if sym.st_size == 0 {
                continue;
            }

            // FIXME maybe the error here should just be a warning instead. I'm pretty sure it's
            // recoverable :|
            let sym_name = if let Some(name) = elf
                .strtab
                .get(sym.st_name)
                .transpose()
                .map_err(|e| Error::new("failed to get symbol name", Box::new(e)))?
            {
                name
            } else {
                continue;
            };

            let (section_offset, section_addr) = {
                let sym_section = elf.section_headers.get(sym.st_shndx).ok_or_else(|| {
                    Error::msg(format!(
                        "no matching section header for {} (header-idx: {})",
                        sym_name, sym.st_shndx
                    ))
                })?;
                (sym_section.sh_offset, sym_section.sh_addr)
            };

            // FIXME clamp values to section bounds.
            // FIXME This works for executable and shared objects that use st_value as a virtual
            // address to the symbol, but I also want to handle relocatable files, in which case
            // st_value would hold a section offset for the symbol.
            let sym_addr = sym.st_value;
            let sym_offset = (sym_addr - section_addr) + section_offset;

            self.symbols.push(Symbol::new(
                sym_name,
                sym_addr,
                sym_offset as usize,
                sym.st_size as usize,
                SymbolType::Function,
                SymbolSource::Elf,
                SymbolLang::Unknown,
            ));
        }

        Ok(())
    }

    fn parse_mach_object(&mut self, mach: &Mach) -> Result<(), Error> {
        Err(Error::msg("mach objects are not currently supported"))
    }

    fn parse_pe_object(&mut self, pe: &PE) -> Result<(), Error> {
        Err(Error::msg("PE objects are not currently supported"))
    }

    fn parse_archive_object(&mut self, archive: &Archive) -> Result<(), Error> {
        Err(Error::msg("archive objects are not currently supported"))
    }

    fn get_elf_section_data_by_name(&self, elf: &Elf, name: &str) -> Result<BinaryData, Error> {
        for section in elf.section_headers.iter() {
            let section_name = elf
                .shdr_strtab
                .get(section.sh_name)
                .transpose()
                .map_err(|err| Error::new("failed to retrieve section name", Box::new(err)))?;

            if section_name == Some(name) {
                let start = section.sh_offset as usize;
                let end = start + section.sh_size as usize;
                return Ok(self.data.slice(start..end));
            }
        }

        Ok(self.data.slice(0..0))
    }
}

/// Reference counted and memory mapped binary data.
#[derive(Clone)]
pub struct BinaryData {
    /// How much of `inner` is visible from this slice of [`BinaryData`].
    range: std::ops::Range<usize>,

    /// The current offset of the binary data that is being read.
    offset: usize,

    inner: Rc<Mmap>,
}

impl BinaryData {
    /// Loads binary data from a file.
    pub fn from_file(file: &File) -> io::Result<Self> {
        unsafe {
            MmapOptions::new().map(&file).map(|m| BinaryData {
                range: 0..m.len(),
                offset: 0,
                inner: Rc::new(m),
            })
        }
    }

    pub fn slice<R>(&self, range: R) -> BinaryData
    where
        R: std::ops::RangeBounds<usize>,
    {
        use std::ops::Bound;

        let start = match range.start_bound() {
            Bound::Included(b) => std::cmp::min(self.range.start + b, self.range.end),
            Bound::Excluded(b) => std::cmp::min(self.range.start + b + 1, self.range.end),
            Bound::Unbounded => self.range.start,
        };

        let end = match range.end_bound() {
            Bound::Included(b) => std::cmp::min(self.range.start + b + 1, self.range.end),
            Bound::Excluded(b) => std::cmp::min(self.range.start + b, self.range.end),
            Bound::Unbounded => self.range.end,
        };

        // advance the offset so that `Read::read` is consistent.
        let offset = self.offset + (start - self.range.start);

        BinaryData {
            range: start..end,
            offset,
            inner: self.inner.clone(),
        }
    }
}

impl std::fmt::Debug for BinaryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinaryData")
            .field("len", &self.inner.len())
            .finish()
    }
}

impl std::ops::Deref for BinaryData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner[self.range.clone()]
    }
}

impl Read for BinaryData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut slice: &[u8] = &*self.inner;

        let len = std::cmp::min(buf.len(), slice.len() - self.offset);
        if len == 0 {
            return Ok(0);
        }
        slice = &slice[self.offset..(self.offset + len)];
        (&mut buf[..len]).copy_from_slice(slice);

        self.offset += len;
        Ok(len)
    }
}

impl Seek for BinaryData {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => (self.len() as i64).saturating_add(offset) as u64,
            SeekFrom::Current(offset) => (self.offset as i64).saturating_add(offset) as u64,
        };

        self.offset = if let Ok(offset) = usize::try_from(offset) {
            std::cmp::min(offset, self.len())
        } else {
            self.len()
        };
        Ok(self.offset as u64)
    }
}

unsafe impl gimli::CloneStableDeref for BinaryData {}
unsafe impl gimli::StableDeref for BinaryData {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Arch {
    Unknown,
    X86,
    X86_64,
    Arm,
    AArch64,
}

impl Arch {
    fn from_elf_machine(machine: u16) -> Arch {
        use goblin::elf::header;

        match machine {
            header::EM_386 => Arch::X86,
            header::EM_X86_64 => Arch::X86_64,
            header::EM_ARM => Arch::Arm,
            header::EM_AARCH64 => Arch::AArch64,
            _ => Arch::Unknown,
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            Arch::Unknown => "unknown",
            Arch::X86 => "x86",
            Arch::X86_64 => "x86_64",
            Arch::Arm => "arm",
            Arch::AArch64 => "arm64",
        };
        write!(f, "{}", t)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Bits {
    Unknown,
    Bits32,
    Bits64,
}

impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            Bits::Unknown => "??-bits",
            Bits::Bits32 => "32-bits",
            Bits::Bits64 => "64-bits",
        };
        write!(f, "{}", t)
    }
}

impl Bits {
    fn from_elf_class(class: u8) -> Bits {
        use goblin::elf::header;

        match class {
            header::ELFCLASS32 => Bits::Bits32,
            header::ELFCLASS64 => Bits::Bits64,
            _ => Bits::Unknown,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Endian {
    Unknown,
    Little,
    Big,
}

impl From<goblin::container::Endian> for Endian {
    fn from(g: goblin::container::Endian) -> Self {
        match g {
            goblin::container::Endian::Little => Endian::Little,
            goblin::container::Endian::Big => Endian::Big,
        }
    }
}

impl From<Endian> for gimli::RunTimeEndian {
    fn from(e: Endian) -> gimli::RunTimeEndian {
        match e {
            Endian::Little => gimli::RunTimeEndian::Little,
            Endian::Big => gimli::RunTimeEndian::Big,

            #[cfg(target_endian = "little")]
            Endian::Unknown => gimli::RunTimeEndian::Little,

            #[cfg(target_endian = "big")]
            Endian::Unknown => gimli::RunTimeEndian::Big,
        }
    }
}

impl fmt::Display for Endian {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            Endian::Unknown => "unknown endian",
            Endian::Little => "little-endian",
            Endian::Big => "big-endian",
        };
        write!(f, "{}", t)
    }
}

/// Names used for detecting DWARF debug information.
const DWARF_SECTIONS: &[&str] = &[
    ".debug_abbrev",
    ".debug_addr",
    ".debug_info",
    ".debug_line",
    ".debug_line_str",
    ".debug_str",
    ".debug_str_offsets",
    ".debug_types",
    ".debug_loc",
    ".debug_loclists",
    ".debug_ranges",
    ".debug_rnglists",
];
