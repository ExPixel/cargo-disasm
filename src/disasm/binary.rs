use super::dwarf::DwarfInfo;
use super::pdb::PDBInfo;
use super::strmatch::{distance, Tokenizer};
use super::symbol::{Symbol, SymbolLang, SymbolSource, SymbolType};
use crate::util;
use anyhow::Context as _;
use goblin::mach::segment::Section as MachSection;
use goblin::{
    archive::Archive,
    elf::Elf,
    mach::{Mach, MachO},
    pe::PE,
    Object,
};
use memmap::{Mmap, MmapOptions};
use std::convert::TryFrom as _;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
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
            Object::PE(pe) => self.parse_pe_object(&pe),
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
        use goblin::elf::header;

        log::debug!("object type   = ELF");

        self.bits = Bits::from_elf_class(elf.header.e_ident[header::EI_CLASS]);
        self.endian = Endian::from(
            elf.header
                .endianness()
                .context("failed to identify ELF endianness")?,
        );
        self.arch = Arch::from_elf_machine(elf.header.e_machine);

        log::debug!("object bits   = {}", self.bits);
        log::debug!("object endian = {}", self.endian);
        log::debug!("object arch   = {}", self.arch);

        let load_all_symbols_timer = std::time::Instant::now();

        let mut load_elf_symbols = false;
        let mut load_dwarf_symbols = options.sources.is_empty(); // `auto` makes this true
        for &source in options.sources.iter() {
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
            self.parse_elf_dwarf(elf, load_dwarf_symbols)?;
        }

        // If we're using `auto` for the symbol source and no symbols are found.
        load_elf_symbols |= options.sources.is_empty() && self.symbols.is_empty();

        if load_elf_symbols {
            log::info!("retrieving symbols from ELF object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            self.gather_elf_symbols(elf)?;
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

    fn parse_elf_dwarf(&mut self, elf: &Elf, load_dwarf_symbols: bool) -> anyhow::Result<()> {
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
            log::info!("retrieving symbols from DWARF debug information");

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
                    .binary_search_by(|(probe, _)| util::cmp_range_to_idx(probe, addr))
                    .ok()
                    .map(|idx| {
                        let &(ref range, off) = &sections[idx];
                        (addr - range.start) as usize + off
                    })
            };

            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            dwarf.load_symbols(&mut self.symbols, addr_to_offset)?;
            log::trace!(
                "found {} symbols in DWARF debug information in {}",
                self.symbols.len() - symbols_count_before,
                util::DurationDisplay(load_symbols_timer.elapsed())
            );
        }
        self.dwarf = Some(dwarf);
        Ok(())
    }

    fn gather_elf_symbols(&mut self, elf: &Elf) -> anyhow::Result<()> {
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
                .context("failed to get ELF symbol name")?
            {
                name
            } else {
                continue;
            };

            let (section_offset, section_addr) = {
                let sym_section = elf.section_headers.get(sym.st_shndx).ok_or_else(|| {
                    anyhow::anyhow!(
                        "no matching section header for {} (header-idx: {})",
                        sym_name,
                        sym.st_shndx
                    )
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

    fn parse_mach_object(&mut self, mach: &MachO, options: SearchOptions) -> anyhow::Result<()> {
        log::debug!("object type   = Mach-O");

        self.bits = if mach.is_64 {
            Bits::Bits64
        } else {
            Bits::Bits32
        };
        self.endian = if mach.little_endian {
            Endian::Little
        } else {
            Endian::Big
        };
        self.arch = Arch::from_mach_cpu_types(mach.header.cputype, mach.header.cpusubtype);

        log::debug!("object bits   = {}", self.bits);
        log::debug!("object endian = {}", self.endian);
        log::debug!("object arch   = {}", self.arch);

        let load_all_symbols_timer = std::time::Instant::now();
        let mut load_mach_symbols = false;
        let mut load_dwarf_symbols = options.sources.is_empty();

        for &source in options.sources.iter() {
            match source {
                SymbolSource::Mach => load_mach_symbols = true,
                SymbolSource::Dwarf => load_dwarf_symbols = true,
                _ => {}
            }
        }

        let mut sections: Vec<MachSection> = Vec::new();
        for segment in mach.segments.iter() {
            for s in segment.into_iter() {
                let (section, _) = s.context("error occured while getting Mach-O section")?;
                sections.push(section);
            }
        }

        self.parse_mach_dwarf(&sections, load_dwarf_symbols)?;

        // If we're using `auto` for the symbol source and no symbols are found.
        load_mach_symbols |= options.sources.is_empty() && self.symbols.is_empty();

        if load_mach_symbols {
            log::info!("retrieving symbols from Mach-O object");
            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            self.gather_mach_symbols(mach, &sections)?;
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

    fn gather_mach_symbols(
        &mut self,
        mach: &MachO,
        sections: &[MachSection],
    ) -> anyhow::Result<()> {
        use goblin::mach::symbols;

        // The starting index for Mach symbols in the `symbols` vector.
        let mach_symbols_idx = self.symbols.len();

        // A list of ALL symbol addresses (even non-function symbols).
        // This will be used for figuring out where symbols end.
        let mut symbol_addresses = Vec::<u64>::with_capacity(32);

        let mut symbols_it = mach.symbols();
        while let Some(Ok((sym_name, sym))) = symbols_it.next() {
            if sym.n_sect == symbols::NO_SECT as usize || !sym.is_stab() {
                continue;
            }

            let sym_addr = sym.n_value;
            symbol_addresses.push(sym_addr);

            if sym.n_type != MACH_TYPE_FUNC || sym_name.is_empty() {
                continue;
            }

            let sym_offset = if let Some(section) = sections.get(sym.n_sect - 1) {
                (sym_addr - section.addr) as usize + section.offset as usize
            } else {
                continue;
            };

            self.symbols.push(Symbol::new(
                sym_name,
                sym_addr,
                sym_offset as usize,
                0, // this is fixed later
                SymbolType::Function,
                SymbolSource::Mach,
                SymbolLang::Unknown,
            ));
        }

        symbol_addresses.sort_unstable();
        symbol_addresses.dedup();

        // Figure out where symbols end by using the starting address of the next symbol.
        for symbol in &mut self.symbols[mach_symbols_idx..] {
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

    fn parse_mach_dwarf(
        &mut self,
        sections: &[MachSection],
        load_dwarf_symbols: bool,
    ) -> anyhow::Result<()> {
        match self.load_dsym_dwarf().context("failed to load mach DWARF") {
            Ok(maybe_dwarf) => self.dwarf = maybe_dwarf,
            Err(err) => log::warn!("{:?}", err),
        }

        // If we don't load a DWARF from an external source, use the current
        // Mach binary.
        if self.dwarf.is_none() {
            let endian = gimli::RunTimeEndian::from(self.endian);
            let loader = |section: gimli::SectionId| {
                Self::get_mach_section_data_by_name(&self.data, &sections, section.name())
                    .map(|d| gimli::EndianReader::new(d, endian))
            };
            let sup_loader = |_section: gimli::SectionId| {
                Ok(gimli::EndianReader::new(self.data.slice(0..0), endian))
            };
            let _dwarf = Box::new(DwarfInfo::new(loader, sup_loader)?);
        }

        if let (true, &Some(ref dwarf)) = (load_dwarf_symbols, &self.dwarf) {
            log::info!("retrieving symbols from DWARF debug information");

            let addr_to_offset = |addr| {
                sections
                    .binary_search_by(|probe| {
                        util::cmp_range_to_idx(
                            &(probe.addr..(probe.addr + probe.size as u64)),
                            addr,
                        )
                    })
                    .ok()
                    .map(|idx| (addr - sections[idx].addr) as usize + sections[idx].offset as usize)
            };

            let symbols_count_before = self.symbols.len();
            let load_symbols_timer = std::time::Instant::now();
            dwarf.load_symbols(&mut self.symbols, addr_to_offset)?;
            log::trace!(
                "found {} symbols in DWARF debug information in {}",
                self.symbols.len() - symbols_count_before,
                util::DurationDisplay(load_symbols_timer.elapsed())
            );
        }
        Ok(())
    }

    /// Find and load the DWARF object file from the dSYM directory.
    fn load_dsym_dwarf(&self) -> anyhow::Result<Option<Box<DwarfInfo>>> {
        let dsym_directory = if let Some(d) = self.find_dsym_directory() {
            d
        } else {
            return Ok(None);
        };
        log::trace!("found dSYM directory: {}", dsym_directory.display());
        let object_path = {
            let mut o_path = dsym_directory;
            o_path.push("Contents");
            o_path.push("Resources");
            o_path.push("DWARF");
            if let Some(fname) = self.data.path().file_name() {
                o_path.push(fname);
            } else {
                return Ok(None);
            }
            o_path
        };

        if !object_path.is_file() {
            log::trace!(
                "did not find dSYM DWARF object file at expected path: {}",
                object_path.display()
            );
            return Ok(None);
        } else {
            log::trace!(
                "located dSYM DWARF object file at {}",
                object_path.display()
            );
        }

        let data =
            BinaryData::from_path(&object_path).context("failed to load Mach-O DWARF binary")?;
        let mach = Mach::parse(&data)
            .with_context(|| format!("failed to parse Mach-O binary {}", object_path.display()))?;
        let mach = match mach {
            goblin::mach::Mach::Fat(multi) => multi
                .get(0)
                .context("failed to get first object from fat Mach binary")?,
            goblin::mach::Mach::Binary(obj) => obj,
        };

        let mut sections: Vec<MachSection> = Vec::new();
        for segment in mach.segments.iter() {
            for s in segment.into_iter() {
                let (section, _) = s.context("error occured while getting Mach-O section")?;
                sections.push(section);
            }
        }
        sections.sort_unstable_by_key(|section| section.addr);

        let endian = if mach.little_endian {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };
        let loader = |section: gimli::SectionId| {
            Self::get_mach_section_data_by_name(&data, &sections, section.name())
                .map(|d| gimli::EndianReader::new(d, endian))
        };
        let sup_loader = |_section: gimli::SectionId| {
            Ok(gimli::EndianReader::new(self.data.slice(0..0), endian))
        };
        let dwarf = Box::new(DwarfInfo::new(loader, sup_loader)?);

        Ok(Some(dwarf))
    }

    /// Find the dSYM directory relative to the currently
    /// loaded executable.
    fn find_dsym_directory(&self) -> Option<PathBuf> {
        let executable_dir = self.data.path().parent()?;
        let entries = executable_dir.read_dir().ok().or_else(|| {
            log::warn!("failed to open `{}` as directory", executable_dir.display());
            None
        })?;

        entries
            .filter_map(|entry| entry.map(|e| e.path()).ok())
            .filter(|path| {
                path.file_name()
                    .filter(|n| n.to_string_lossy().ends_with(".dSYM"))
                    .is_some()
            })
            .find(|path| path.is_dir())
    }

    fn parse_pe_object(&mut self, _pe: &PE) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("PE objects are not currently supported"))
    }

    fn parse_archive_object(&mut self, _archive: &Archive) -> anyhow::Result<()> {
        Err(anyhow::anyhow!(
            "archive objects are not currently supported"
        ))
    }

    fn get_mach_section_data_by_name(
        data: &BinaryData,
        sections: &[MachSection],
        name: &str,
    ) -> anyhow::Result<BinaryData> {
        let dot = name.starts_with('.');

        if let Some(section) = sections.iter().find(|section| {
            if let Ok(section_name) = section.name() {
                if section_name.starts_with("__") && dot {
                    section_name[2..] == name[1..]
                } else {
                    section_name == name
                }
            } else {
                false
            }
        }) {
            let start = section.offset as usize;
            let end = start + section.size as usize;
            Ok(data.slice(start..end))
        } else {
            Ok(data.slice(0..0))
        }
    }

    fn get_elf_section_data_by_name(&self, elf: &Elf, name: &str) -> anyhow::Result<BinaryData> {
        for section in elf.section_headers.iter() {
            let section_name = elf
                .shdr_strtab
                .get(section.sh_name)
                .transpose()
                .context("failed to retrieve ELF section name")?;

            if section_name == Some(name) {
                let start = section.sh_offset as usize;
                let end = start + section.sh_size as usize;
                return Ok(self.data.slice(start..end));
            }
        }

        Ok(self.data.slice(0..0))
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

const MACH_TYPE_FUNC: u8 = 0x24;

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
