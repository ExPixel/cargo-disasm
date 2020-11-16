mod elf;
mod mach;
mod pe;

use super::dwarf::DwarfInfo;
use super::pdb::PDBInfo;
use super::strmatch::{distance, Tokenizer};
use super::symbol::{Symbol, SymbolLang, SymbolSource, SymbolType};
use crate::util;
use anyhow::Context as _;

use goblin::{archive::Archive, elf::Elf, mach::MachO, pe::PE, Object};
use memmap::{Mmap, MmapOptions};
use std::convert::TryFrom as _;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Threshold for using all available sources when symbol sources is 'auto'.
/// While there are less than `AUTO_SOURCES_THRESHOLD` symbols loaded
/// and symsrc is `auto`, the binary may keep loading more sources.
const AUTO_SOURCES_THRESHOLD: usize = 128 * 1024;

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
    pub fn new(data: BinaryData, options: SearchOptions) -> anyhow::Result<Binary> {
        let mut binary = Binary {
            data,
            dwarf: None,
            pdb: None,

            arch: Arch::Unknown,
            endian: Endian::Unknown,
            bits: Bits::Unknown,

            symbols: Vec::new(),
        };

        binary.parse_object(options).map(|_| {
            let symbol_sort_timer = std::time::Instant::now();
            binary.symbols.sort_unstable_by(|lhs, rhs| {
                lhs.address()
                    .cmp(&rhs.address())
                    .then(lhs.end_address().cmp(&rhs.end_address()))
            });
            log::trace!(
                "sorted {} symbols in {}",
                binary.symbols.len(),
                util::DurationDisplay(symbol_sort_timer.elapsed())
            );

            binary
        })
    }

    /// Returns a symbol (and offset) for an address.
    pub fn symbolicate(&self, addr: u64) -> Option<(&Symbol, u64)> {
        let mut idx = self
            .symbols
            .binary_search_by(|probe| util::cmp_range_to_idx(&probe.address_range(), addr))
            .ok()?;

        // We might have duplicates of a symbol (e.g. from DWARF and ELF), so we want
        // to scan backwards until we find the one with the highest priority.
        while idx > 0 {
            if self.symbols[idx - 1].address_range().contains(&addr) {
                idx -= 1;
            } else {
                break;
            }
        }

        self.symbols.get(idx).map(|sym| (sym, addr - sym.address()))
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
                distance(
                    tokens.iter().copied(),
                    Tokenizer::new(&sym.name()),
                    u32::MAX,
                )?,
                sym,
            ))
        })
    }

    pub fn fuzzy_find_symbol<'s>(&'s self, name: &str) -> Option<&'s Symbol> {
        let tokens = Tokenizer::new(name).collect::<Vec<&str>>();
        let symbol_search_timer = std::time::Instant::now();

        let mut smallest_distance = std::u32::MAX;
        let symbol = self
            .symbols
            .iter()
            .filter_map(|sym| {
                let dist = distance(
                    tokens.iter().copied(),
                    Tokenizer::new(&sym.name()),
                    smallest_distance,
                )?;

                if dist < smallest_distance {
                    smallest_distance = dist;
                }

                Some((dist, sym))
            })
            .min_by(|lhs, rhs| {
                lhs.0
                    .cmp(&rhs.0)
                    .then_with(|| lhs.1.source().cmp(&rhs.1.source()))
                    .then_with(|| lhs.1.address().cmp(&rhs.1.address()))
                    .then_with(|| lhs.1.offset().cmp(&rhs.1.offset()))
                    .then_with(|| lhs.1.name().cmp(&rhs.1.name()))
            })
            .map(|(_, sym)| sym);

        log::trace!(
            "fuzzy matched `{}` in {}",
            name,
            util::DurationDisplay(symbol_search_timer.elapsed())
        );
        symbol
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

    fn parse_object(&mut self, options: SearchOptions) -> anyhow::Result<()> {
        let data = self.data.clone();
        match Object::parse(&data).context("failed to parse object")? {
            Object::Elf(elf) => self.parse_elf_object(&elf, options),
            Object::PE(pe) => self.parse_pe_object(&pe, options),
            Object::Mach(mach) => match mach {
                goblin::mach::Mach::Fat(multi) => self.parse_mach_object(
                    &multi
                        .get(0)
                        .context("failed to get first object from fat Mach binary")?,
                    options,
                ),
                goblin::mach::Mach::Binary(obj) => self.parse_mach_object(&obj, options),
            },
            Object::Archive(archive) => self.parse_archive_object(&archive),
            Object::Unknown(magic) => Err(anyhow::anyhow!(
                "failed to parse object with magic value 0x{:X}",
                magic
            )),
        }
    }

    fn parse_elf_object(&mut self, elf: &Elf, options: SearchOptions) -> anyhow::Result<()> {
        elf::load_arch_info(self, elf)?;

        let load_all_symbols_timer = std::time::Instant::now();
        let mut load_elf_symbols = false;
        let mut load_dwarf_symbols = options.sources.is_empty(); // `auto` makes this true
        options.sources.iter().for_each(|source| match source {
            SymbolSource::Elf => load_elf_symbols = true,
            SymbolSource::Dwarf => load_dwarf_symbols = true,
            _ => {}
        });

        if elf::contains_dwarf(elf) {
            let dwarf = elf::load_dwarf(elf, self.endian, &self.data)?;
            if load_dwarf_symbols {
                log::info!("retrieving symbols from DWARF debug information");
                let symbols_count_before = self.symbols.len();
                let load_symbols_timer = std::time::Instant::now();

                elf::load_dwarf_symbols(&dwarf, elf, &mut self.symbols)?;

                log::trace!(
                    "found {} symbols in DWARF debug information in {}",
                    self.symbols.len() - symbols_count_before,
                    util::DurationDisplay(load_symbols_timer.elapsed())
                );
            }
            self.dwarf = Some(dwarf);
        }

        // If we're using `auto` for the symbol source and no symbols are found.
        load_elf_symbols |=
            options.sources.is_empty() && self.symbols.len() < AUTO_SOURCES_THRESHOLD;

        if load_elf_symbols {
            log::info!("retrieving symbols from ELF object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            elf::load_symbols(elf, &mut self.symbols)?;
            log::trace!(
                "found {} symbols in ELF object in {}",
                self.symbols.len() - symbols_count_before,
                util::DurationDisplay(load_symbols_timer.elapsed())
            );
        }

        log::debug!(
            "found {} total symbols in {}",
            self.symbols.len(),
            util::DurationDisplay(load_all_symbols_timer.elapsed())
        );

        Ok(())
    }

    fn parse_mach_object(&mut self, mach: &MachO, options: SearchOptions) -> anyhow::Result<()> {
        mach::load_arch_info(self, mach)?;

        let load_all_symbols_timer = std::time::Instant::now();
        let mut load_mach_symbols = false;
        let mut load_dwarf_symbols = options.sources.is_empty(); // `auto` makes this true
        options.sources.iter().for_each(|source| match source {
            SymbolSource::Mach => load_mach_symbols = true,
            SymbolSource::Dwarf => load_dwarf_symbols = true,
            _ => {}
        });

        let sections = mach::load_sections(mach)?;

        if let Some(dwarf) = mach::load_dwarf(&sections, self.endian, &self.data)? {
            if load_dwarf_symbols {
                log::info!("retrieving symbols from DWARF debug information");
                let symbols_count_before = self.symbols.len();
                let load_symbols_timer = std::time::Instant::now();

                mach::load_dwarf_symbols(&dwarf, &sections, &mut self.symbols)?;

                log::trace!(
                    "found {} symbols in DWARF debug information in {}",
                    self.symbols.len() - symbols_count_before,
                    util::DurationDisplay(load_symbols_timer.elapsed())
                );
            }
            self.dwarf = Some(dwarf);
        }

        // If we're using `auto` for the symbol source and no symbols are found.
        load_mach_symbols |=
            options.sources.is_empty() && self.symbols.len() < AUTO_SOURCES_THRESHOLD;

        if load_mach_symbols {
            log::info!("retrieving symbols from Mach-O object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            mach::load_symbols(mach, &sections, &mut self.symbols)?;
            log::trace!(
                "found {} symbols in Mach-O object in {}",
                self.symbols.len() - symbols_count_before,
                util::DurationDisplay(load_symbols_timer.elapsed())
            );
        }

        log::debug!(
            "found {} total symbols in {}",
            self.symbols.len(),
            util::DurationDisplay(load_all_symbols_timer.elapsed())
        );

        Ok(())
    }

    fn parse_pe_object(&mut self, pe: &PE, options: SearchOptions) -> anyhow::Result<()> {
        log::debug!("object type   = PE/COFF");

        self.bits = if pe.is_64 { Bits::Bits64 } else { Bits::Bits32 };
        self.endian = Endian::Little;
        self.arch = Arch::from_coff_machine(pe.header.coff_header.machine);

        log::debug!("object bits   = {}", self.bits);
        log::debug!("object endian = {}", self.endian);
        log::debug!("object arch   = {}", self.arch);

        let load_all_symbols_timer = std::time::Instant::now();
        let mut load_pe_symbols = false;
        let mut load_pdb_symbols = options.sources.is_empty();
        let mut load_dwarf_symbols = options.sources.is_empty();
        options.sources.iter().for_each(|source| match source {
            SymbolSource::Pe => load_pe_symbols = true,
            SymbolSource::Pdb => load_pdb_symbols = true,
            SymbolSource::Dwarf => load_dwarf_symbols = true,
            _ => {}
        });

        self.parse_pe_pdb(pe, load_pdb_symbols)
            .context("error while gathering PDB symbols")?;
        self.parse_pe_dwarf(pe, load_dwarf_symbols)
            .context("error while gathering DWARF symbols")?;

        // If we're using `auto` for the symbol source and no symbols are found.
        load_pe_symbols |=
            options.sources.is_empty() && self.symbols.len() < AUTO_SOURCES_THRESHOLD;

        if load_pe_symbols {
            log::info!("retrieving symbols from PE/COFF object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            self.gather_pe_symbols(pe)
                .context("error while gathering PE symbols")?;
            log::trace!(
                "found {} symbols in PE/COFF object in {}",
                self.symbols.len() - symbols_count_before,
                util::DurationDisplay(load_symbols_timer.elapsed())
            );
        }

        log::debug!(
            "found {} total symbols in {}",
            self.symbols.len(),
            util::DurationDisplay(load_all_symbols_timer.elapsed())
        );

        Ok(())
    }

    fn gather_pe_symbols(&mut self, pe: &PE) -> anyhow::Result<()> {
        use goblin::pe;

        #[rustfmt::skip]
        let symtab = pe.header.coff_header.symbols(&self.data)
            .context("error while loading COFF header symbol table")?;

        // There are no symbols in here >:(
        if symtab.get(0).is_none() {
            log::debug!("no symbols in PE/COFF object");
            return Ok(());
        }

        #[rustfmt::skip]
        let strtab = pe.header.coff_header.strings(&self.data)
            .context("error while loading COFF header string table")?;
        let pe_symbols_index = self.symbols.len();

        // A list of ALL symbol addresses (even non-function symbols).
        // This will be used for figuring out where symbols end.
        let mut symbol_addresses = Vec::<u64>::with_capacity(32);

        for (_sym_index, inline_name, symbol) in symtab.iter() {
            let (sym_addr, sym_offset) = if symbol.section_number >= 1 {
                let section = &pe.sections[symbol.section_number as usize - 1];

                if symbol.storage_class == pe::symbol::IMAGE_SYM_CLASS_STATIC
                    || symbol.storage_class == pe::symbol::IMAGE_SYM_CLASS_EXTERNAL
                    || symbol.storage_class == pe::symbol::IMAGE_SYM_CLASS_LABEL
                {
                    (
                        pe.image_base as u64 + (section.virtual_address + symbol.value) as u64,
                        (section.pointer_to_raw_data + symbol.value) as usize,
                    )
                } else {
                    continue;
                }
            } else {
                continue;
            };

            symbol_addresses.push(sym_addr);

            if !symbol.is_function_definition() {
                continue;
            }

            // FIXME for now we skip symbols that are just sections but I think sections can also
            // actually contain functions in which case the entire section should be used. I'm not
            // sure if this is the case though.
            if symbol.value == 0 {
                continue;
            }

            let sym_name = if let Some(name) = inline_name {
                name
            } else if let Some(Ok(name)) = symbol
                .name_offset()
                .and_then(|off| strtab.get(off as usize))
            {
                name
            } else {
                continue;
            };

            self.symbols.push(Symbol::new(
                sym_name,
                sym_addr,
                sym_offset as usize,
                0, // this is fixed later
                SymbolType::Function,
                SymbolSource::Pe,
                SymbolLang::Unknown,
            ));
        }

        symbol_addresses.sort_unstable();
        symbol_addresses.dedup();

        // Figure out where symbols end by using the starting address of the next symbol.
        for symbol in &mut self.symbols[pe_symbols_index..] {
            if let Ok(idx) = symbol_addresses.binary_search(&symbol.address()) {
                if let Some(next_addr) = symbol_addresses.get(idx + 1) {
                    symbol.set_size((next_addr - symbol.address()) as usize);
                    continue;
                }
            };
            symbol.set_address(0);
        }

        Ok(())
    }

    fn parse_pe_pdb(&mut self, pe: &PE, _load_symbols: bool) -> anyhow::Result<()> {
        self.load_pe_pdb(pe)?;
        Ok(())
    }

    fn parse_pe_dwarf(&mut self, _pe: &PE, _load_symbols: bool) -> anyhow::Result<()> {
        Ok(())
    }

    fn load_pe_pdb(&mut self, pe: &PE) -> anyhow::Result<Option<Box<PDBInfo>>> {
        let path = if let Some(path) = self.get_pe_pdb_path(pe)? {
            path
        } else {
            return Ok(None);
        };
        log::debug!("found PDB at `{}`", path.display());
        Ok(None)
    }

    fn get_pe_pdb_path(&self, pe: &PE) -> anyhow::Result<Option<PathBuf>> {
        if let Some(ref debug_path) = pe
            .debug_data
            .as_ref()
            .and_then(|data| data.codeview_pdb70_debug_info.as_ref())
            .and_then(|cv| std::ffi::CStr::from_bytes_with_nul(cv.filename).ok())
            .and_then(|cs| cs.to_str().ok())
        {
            let path = Path::new(debug_path);
            if path.is_absolute() && path.is_file() {
                Ok(Some(path.into()))
            } else {
                Ok(debug_path
                    .rsplit(|c| c == '/' || c == '\\')
                    .next()
                    .map(|s| Path::new(s))
                    .or_else(|| Some(Path::new(self.data.path().file_stem()?)))
                    .and_then(|p| Some(self.data.path().parent()?.join(p)))
                    .filter(|p| {
                        if p.is_file() {
                            true
                        } else {
                            log::debug!("did not find PDB at expected path `{}`", p.display());
                            false
                        }
                    }))
            }
        } else {
            log::debug!("here");
            // This closure if here just to simplify handling the 2 None cases.
            let get_path = || -> Option<PathBuf> {
                let mut buf = PathBuf::from(self.data.path().parent()?);
                let mut name = self.data.path().file_stem()?.to_owned();
                name.push(".pdb");
                buf.push(name);
                if buf.is_file() {
                    Some(buf)
                } else {
                    log::debug!("did not find PDB at expected path `{}`", buf.display());
                    None
                }
            };

            Ok(get_path())
        }
    }

    fn parse_archive_object(&mut self, _archive: &Archive) -> anyhow::Result<()> {
        Err(anyhow::anyhow!(
            "archive objects are not currently supported"
        ))
    }
}

struct BinaryDataInner {
    /// The mapped memory for this binary data.
    mmap: Mmap,

    /// The original path that was used to load this binary data.
    path: PathBuf,

    /// The file that was used to load this binary data.
    file: File,
}

/// Reference counted and memory mapped binary data.
#[derive(Clone)]
pub struct BinaryData {
    /// How much of `inner` is visible from this slice of [`BinaryData`].
    range: std::ops::Range<usize>,

    /// The current offset of the binary data that is being read.
    offset: usize,

    inner: Rc<BinaryDataInner>,
}

impl BinaryData {
    pub fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Self::from_path_inner(path.as_ref())
    }

    fn from_path_inner(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open file at path `{}`", path.display()))?;
        let path = PathBuf::from(path);

        unsafe {
            MmapOptions::new()
                .map(&file)
                .map(|mmap| BinaryData {
                    range: 0..mmap.len(),
                    offset: 0,
                    inner: Rc::new(BinaryDataInner { mmap, file, path }),
                })
                .map_err(|err| err.into())
        }
    }

    /// Returns the original path used to load this binary data if one
    /// was provided.
    pub fn path(&self) -> &Path {
        &self.inner.path
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
            .field("len", &self.inner.mmap.len())
            .finish()
    }
}

impl std::ops::Deref for BinaryData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner.mmap[self.range.clone()]
    }
}

impl Read for BinaryData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut slice: &[u8] = &*self.inner.mmap;

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

    fn from_mach_cpu_types(cpu_type: u32, _cpu_subtype: u32) -> Arch {
        use goblin::mach::constants::cputype;

        match cpu_type {
            cputype::CPU_TYPE_ARM => Arch::Arm,
            cputype::CPU_TYPE_ARM64 => Arch::AArch64,
            cputype::CPU_TYPE_ARM64_32 => Arch::AArch64,
            cputype::CPU_TYPE_X86 => Arch::X86,
            cputype::CPU_TYPE_X86_64 => Arch::X86_64,
            _ => Arch::Unknown,
        }
    }

    fn from_coff_machine(machine: u16) -> Arch {
        use goblin::pe::header;

        match machine {
            header::COFF_MACHINE_X86 => Arch::X86,
            header::COFF_MACHINE_X86_64 => Arch::X86_64,
            header::COFF_MACHINE_ARM => Arch::Arm,
            header::COFF_MACHINE_ARM64 => Arch::AArch64,
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

pub struct SearchOptions<'a> {
    pub sources: &'a [SymbolSource],

    /// Path to an object file containing DWARF debug information.
    /// Used for ELF and Mach-O object files.
    pub dwarf_path: Option<&'a Path>,

    /// The path to the dSYM directory.
    /// Used for Mach-O object files.
    pub dsym_path: Option<&'a Path>,

    /// Path to a PDB file used for PE object files.
    pub pdb_path: Option<&'a Path>,
}
