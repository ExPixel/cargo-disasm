mod elf;
mod mach;
mod pe;

use super::dwarf::DwarfInfo;
use super::pdb::PDBInfo;
use super::strmatch::{distance, Tokenizer};
use super::symbol::{Symbol, SymbolSource};
use crate::util;
use anyhow::Context as _;

use goblin::{archive::Archive, elf::Elf, mach::MachO, pe::PE, Object};
use memmap::{Mmap, MmapOptions};
use std::convert::TryFrom as _;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

                elf::load_dwarf_symbols(&dwarf, elf, &mut self.symbols)
                    .context("error while gather DWARF symbols")?;

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
            elf::load_symbols(elf, &mut self.symbols)
                .context("error while gathering ELF symbols")?;
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
                mach::load_dwarf_symbols(&dwarf, &sections, &mut self.symbols)
                    .context("error while gathering DWARF symbols")?;
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
            mach::load_symbols(mach, &sections, &mut self.symbols)
                .context("error while gathering Mach symbols")?;
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
        pe::load_arch_info(self, pe)?;

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

        if let Some(pdb_path) =
            pe::find_pdb_path(pe, self.data.path()).context("error while searching for PDB")?
        {
            log::debug!("found PDB at `{}`", pdb_path.display());
            let pdb_data =
                BinaryData::from_path(pdb_path).context("error while loading PDB data")?;
            let mut pdb = pe::load_pdb(pdb_data)?;
            if load_pdb_symbols {
                log::info!("retrieving symbols from PDB debug information");
                let symbols_count_before = self.symbols.len();
                let load_symbols_timer = std::time::Instant::now();
                pe::load_pdb_symbols(pe, &mut pdb, &mut self.symbols)
                    .context("error while gather PDB symbols")?;
                log::trace!(
                    "found {} symbols in PDB debug information in {}",
                    self.symbols.len() - symbols_count_before,
                    util::DurationDisplay(load_symbols_timer.elapsed())
                );
            }
            self.pdb = Some(pdb);
        }

        if pe::contains_dwarf(pe) {
            let dwarf = pe::load_dwarf(pe, self.endian, &self.data)?;
            // If we're using `auto` for the symbol source and no symbols are found.
            load_dwarf_symbols |=
                options.sources.is_empty() && self.symbols.len() < AUTO_SOURCES_THRESHOLD;

            if load_dwarf_symbols {
                let symbols_count_before = self.symbols.len();
                let load_symbols_timer = std::time::Instant::now();
                log::info!("retrieving symbols from DWARF debug information");

                pe::load_dwarf_symbols(&dwarf, pe, &mut self.symbols)
                    .context("error while gather DWARF symbols")?;

                log::trace!(
                    "found {} symbols in DWARF debug information in {}",
                    self.symbols.len() - symbols_count_before,
                    util::DurationDisplay(load_symbols_timer.elapsed())
                );
            }
            self.dwarf = Some(dwarf);
        }

        // If we're using `auto` for the symbol source and no symbols are found.
        load_pe_symbols |=
            options.sources.is_empty() && self.symbols.len() < AUTO_SOURCES_THRESHOLD;

        if load_pe_symbols {
            log::info!("retrieving symbols from PE/COFF object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            pe::load_symbols(pe, &self.data, &mut self.symbols)
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

    fn parse_archive_object(&mut self, _archive: &Archive) -> anyhow::Result<()> {
        Err(anyhow::anyhow!(
            "archive objects are not currently supported"
        ))
    }

    pub fn load_line_information(&mut self) -> anyhow::Result<()> {
        if let Some(ref mut dwarf) = self.dwarf {
            dwarf.ensure_compilation_units()?;
        }

        Ok(())
    }

    pub fn addr2line(
        &self,
        addr: u64,
    ) -> anyhow::Result<Option<impl '_ + Iterator<Item = (&Path, u32)>>> {
        if let Some(ref dwarf) = self.dwarf {
            return dwarf.addr2line(addr);
        }

        Ok(None)
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

    inner: Arc<BinaryDataInner>,
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
                    inner: Arc::new(BinaryDataInner { mmap, file, path }),
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
