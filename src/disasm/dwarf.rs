use crate::disasm::binary::BinaryData;
use crate::disasm::symbol::{Symbol, SymbolSource};
use crate::util;
use anyhow::Context as _;
use gimli::{read::EndianReader, Dwarf, RunTimeEndian};
use once_cell::unsync::OnceCell;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub type BinaryDataReader = EndianReader<RunTimeEndian, BinaryData>;

/// Maps an address range to a compilation unit index.
type UnitRange = (Range<u64>, usize);

pub struct DwarfInfo {
    dwarf: Dwarf<BinaryDataReader>,

    compilation_unit_ranges: Vec<UnitRange>,
    compilation_units: Vec<LazyCompilationUnit>,
    compilation_units_initialized: bool,
}

impl DwarfInfo {
    pub fn new<L, S>(loader: L, sup_loader: S) -> anyhow::Result<DwarfInfo>
    where
        L: Fn(gimli::SectionId) -> anyhow::Result<BinaryDataReader>,
        S: Fn(gimli::SectionId) -> anyhow::Result<BinaryDataReader>,
    {
        Ok(DwarfInfo {
            dwarf: gimli::Dwarf::load(loader, sup_loader)?,

            compilation_unit_ranges: Vec::new(),
            compilation_units: Vec::new(),
            compilation_units_initialized: false,
        })
    }

    /// Loads DWARF symbols into the given output vector.
    pub fn load_symbols<F>(
        &self,
        symbols: &mut Vec<Symbol>,
        addr_to_offset: F,
    ) -> anyhow::Result<()>
    where
        F: Send + Sync + Fn(u64) -> Option<usize>,
    {
        let mut unit_headers = self.dwarf.units();

        let mut units = Vec::new();
        while let Some(unit_header) = match unit_headers.next() {
            Ok(maybe_unit_header) => maybe_unit_header,
            Err(err) => {
                log::debug!("soft error while reading DWARF compilation units: {}", err);
                None
            }
        } {
            if let Ok(unit) = self.dwarf.unit(unit_header) {
                units.push(unit);
            } else {
                continue;
            }
        }

        use rayon::prelude::*;

        log::debug!(
            "processing {} DWARF compilation units using rayon",
            units.len()
        );
        let (result_send, result_recv) =
            std::sync::mpsc::sync_channel::<Result<(), anyhow::Error>>(units.len());
        let dwarf = &self.dwarf;
        symbols.par_extend(units.par_iter().flat_map(move |unit| {
            let mut name_chain = NameChain::new();
            let mut symbols = Vec::with_capacity(32);
            result_send
                .send(
                    Self::load_symbols_from_unit(
                        dwarf,
                        &unit,
                        &mut symbols,
                        &addr_to_offset,
                        &mut name_chain,
                    )
                    .context("failed to load symbols from compilation unit"),
                )
                .expect("receiver should be available");
            symbols
        }));

        // Handle any errors that we encountered while gathering symbols
        for result in result_recv {
            result?;
        }

        Ok(())
    }

    fn load_symbols_from_unit<F>(
        dwarf: &Dwarf<BinaryDataReader>,
        unit: &gimli::Unit<BinaryDataReader>,
        symbols: &mut Vec<Symbol>,
        addr_to_offset: &F,
        name_chain: &mut NameChain,
    ) -> Result<(), gimli::Error>
    where
        F: Fn(u64) -> Option<usize>,
    {
        let mut entries = unit.entries_raw(None)?;

        while !entries.is_empty() {
            name_chain.set_depth(entries.next_depth());

            let abbrev = if let Some(abbrev) = entries.read_abbreviation()? {
                abbrev
            } else {
                continue;
            };

            // // FIXME maybe we should handle inline subroutines as well so that they can
            // //       be properly symbolicated. :\
            if abbrev.tag() == gimli::DW_TAG_subprogram {
                if let Some(symbol) = Self::symbol_from_attributes(
                    abbrev.attributes(),
                    &mut entries,
                    unit,
                    &dwarf,
                    addr_to_offset,
                    name_chain,
                )? {
                    symbols.push(symbol);
                }
            } else {
                const TAGS: &[gimli::DwTag] = &[
                    gimli::DW_TAG_module,
                    gimli::DW_TAG_namespace,
                    gimli::DW_TAG_structure_type,
                    gimli::DW_TAG_class_type,
                    gimli::DW_TAG_union_type,
                    gimli::DW_TAG_interface_type,
                    // FIXME I'm not sure about this one:
                    gimli::DW_TAG_inheritance,
                    gimli::DW_TAG_enumeration_type,
                ];
                let track_name = TAGS.contains(&abbrev.tag());

                // skip the attributes for this DIE.
                for spec in abbrev.attributes() {
                    let attr = entries.read_attribute(*spec)?;

                    // If the name should be tracked, push it onto the name chain.
                    if track_name && attr.name() == gimli::DW_AT_name {
                        name_chain.push(dwarf.attr_string(unit, attr.value())?);
                    }
                }
            }
        }

        Ok(())
    }

    fn symbol_from_attributes<F>(
        attributes: &[gimli::read::AttributeSpecification],
        entries: &mut gimli::read::EntriesRaw<BinaryDataReader>,
        unit: &gimli::Unit<BinaryDataReader>,
        dwarf: &Dwarf<BinaryDataReader>,
        addr_to_offset: &F,
        name_chain: &mut NameChain,
    ) -> Result<Option<Symbol>, gimli::Error>
    where
        F: Fn(u64) -> Option<usize>,
    {
        let mut start = None;
        let mut end: Option<u64> = None;
        let mut name = None;
        let mut linkage_name = false;
        let mut end_is_offset = false;

        for spec in attributes {
            let attr = entries.read_attribute(*spec)?;
            match attr.name() {
                gimli::DW_AT_low_pc => start = dwarf.attr_address(unit, attr.value())?,
                gimli::DW_AT_high_pc => {
                    if let Some(end_addr) = dwarf.attr_address(unit, attr.value())? {
                        end = Some(end_addr);
                    } else if let Some(end_offset) = attr.udata_value() {
                        end = Some(end_offset);
                        end_is_offset = true;
                    }
                }

                // FIXME Here we use the mangled name because I couldn't figure out
                //       how to retrieve a fully qualified name (module::submodule::Type::function)
                //       using DW_AT_name. Maybe this is the right way to do it?
                gimli::DW_AT_linkage_name if name.is_none() => {
                    linkage_name = true;
                    name = Some(dwarf.attr_string(unit, attr.value())?)
                }
                gimli::DW_AT_name => {
                    linkage_name = false;
                    name = Some(dwarf.attr_string(unit, attr.value())?)
                }
                _ => continue,
            }
        }

        if let (Some(start), Some(mut end), Some(name)) = (start, end, name) {
            if end_is_offset {
                end += start;
            }
            let end = end; // FREEZE!

            if let Some(off) = addr_to_offset(start) {
                let len = (end - start) as usize;

                if linkage_name {
                    if let Ok(name) = std::str::from_utf8(name.bytes()) {
                        Ok(Some(Symbol::new(
                            name.to_string(),
                            start,
                            off,
                            len,
                            SymbolSource::Dwarf,
                        )))
                    } else {
                        Ok(None)
                    }
                } else {
                    name_chain.push(name);
                    Ok(Some(Symbol::new_unmangled(
                        name_chain.combine("::"),
                        start,
                        off,
                        len,
                        SymbolSource::Dwarf,
                    )))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// This will load the compilation units and their addresses ranges
    /// if it has not been done already.
    pub fn ensure_compilation_units(&mut self) -> anyhow::Result<()> {
        if self.compilation_units_initialized {
            return Ok(());
        }
        self.compilation_units_initialized = true;

        log::debug!("loading DWARF line information");
        let load_line_info_timer = std::time::Instant::now();

        Self::find_compilation_units(
            &self.dwarf,
            &mut self.compilation_units,
            &mut self.compilation_unit_ranges,
        )
        .context("error while finding compilation units")?;

        log::trace!(
            "loaded {} DWARF compilation unit ranges in {}",
            self.compilation_unit_ranges.len(),
            util::DurationDisplay(load_line_info_timer.elapsed())
        );

        Ok(())
    }

    #[cold]
    fn find_compilation_units(
        dwarf: &Dwarf<BinaryDataReader>,
        units: &mut Vec<LazyCompilationUnit>,
        ranges: &mut Vec<UnitRange>,
    ) -> Result<(), gimli::Error> {
        let compilation_unit_search_timer = std::time::Instant::now();
        let mut unit_headers = dwarf.units();

        while let Some(unit_header) = match unit_headers.next() {
            Ok(maybe_unit_header) => maybe_unit_header,
            Err(err) => {
                log::debug!("soft error while reading DWARF compilation units: {}", err);
                None
            }
        } {
            let unit = if let Ok(unit) = dwarf.unit(unit_header) {
                unit
            } else {
                continue;
            };

            Self::add_compilation_unit(unit, dwarf, units, ranges)?;
        }

        log::trace!(
            "found {} compilation units in {}",
            units.len(),
            util::DurationDisplay(compilation_unit_search_timer.elapsed())
        );

        ranges.sort_unstable_by_key(|r| r.0.start);
        Ok(())
    }

    fn add_compilation_unit(
        unit: gimli::Unit<BinaryDataReader>,
        dwarf: &Dwarf<BinaryDataReader>,
        units: &mut Vec<LazyCompilationUnit>,
        unit_ranges: &mut Vec<UnitRange>,
    ) -> Result<(), gimli::Error> {
        let mut entries = unit.entries_raw(None)?;

        let abbrev = match entries.read_abbreviation()? {
            Some(abbrev) if abbrev.tag() == gimli::DW_TAG_compile_unit => abbrev,
            _ => return Ok(()),
        };

        let mut start_addr = None;
        let mut end_addr = None;
        let mut size = None;
        let mut ranges = None;
        let mut lang = None;

        for spec in abbrev.attributes() {
            let attr = entries.read_attribute(*spec)?;

            match attr.name() {
                gimli::DW_AT_low_pc => {
                    if let gimli::AttributeValue::Addr(val) = attr.value() {
                        start_addr = Some(val);
                    }
                }

                gimli::DW_AT_high_pc => {
                    if let gimli::AttributeValue::Addr(val) = attr.value() {
                        end_addr = Some(val);
                    } else if let Some(val) = attr.udata_value() {
                        size = Some(val);
                    }
                }

                gimli::DW_AT_ranges => {
                    ranges = dwarf.attr_ranges_offset(&unit, attr.value())?;
                }

                gimli::DW_AT_language => {
                    if let gimli::AttributeValue::Language(val) = attr.value() {
                        lang = Some(val);
                    }
                }

                _ => { /* NOP */ }
            }
        }

        let unit_index = units.len();
        if let Some(offset) = ranges {
            let mut ranges = dwarf.ranges(&unit, offset)?;
            while let Some(range) = ranges.next()? {
                unit_ranges.push((range.begin..range.end, unit_index));
            }
        } else if let (Some(begin), Some(end)) = (start_addr, end_addr) {
            unit_ranges.push((begin..end, unit_index));
        } else if let (Some(begin), Some(size)) = (start_addr, size) {
            unit_ranges.push((begin..(begin + size), unit_index));
        }

        units.push(LazyCompilationUnit::new(unit, lang));
        Ok(())
    }

    pub fn addr2line(
        &self,
        addr: u64,
    ) -> anyhow::Result<Option<impl '_ + Iterator<Item = (&Path, u32)>>> {
        let range_idx = if let Ok(idx) = self
            .compilation_unit_ranges
            .binary_search_by(|&(ref probe, _)| util::cmp_range_to_idx(probe, addr))
        {
            idx
        } else {
            return Ok(None);
        };
        let unit_idx = self.compilation_unit_ranges[range_idx].1 as usize;
        let unit = &self.compilation_units[unit_idx];
        let lines = unit.lines(&self.dwarf)?;
        Ok(lines.lines_for_addr(addr))
    }
}

pub struct LazyCompilationUnit {
    unit: gimli::Unit<BinaryDataReader>,

    // FIXME use this for syntax hilighting maybe...or just remove it.
    #[allow(dead_code)]
    lang: Option<gimli::DwLang>,

    lines: OnceCell<Lines>,
}

impl LazyCompilationUnit {
    pub fn new(
        unit: gimli::Unit<BinaryDataReader>,
        lang: Option<gimli::DwLang>,
    ) -> LazyCompilationUnit {
        LazyCompilationUnit {
            unit,
            lang,
            lines: OnceCell::new(),
        }
    }

    fn lines(&self, dwarf: &Dwarf<BinaryDataReader>) -> Result<&Lines, gimli::Error> {
        self.lines.get_or_try_init(|| {
            let load_lines_timer = std::time::Instant::now();
            let lines = self.load_lines(dwarf);
            if let Ok(ref lines) = lines {
                log::trace!(
                    "loaded {} sequences and {} files from DWARF debug information in {}",
                    lines.sequences.len(),
                    lines.files.len(),
                    util::DurationDisplay(load_lines_timer.elapsed())
                );
            }
            lines
        })
    }

    fn load_lines(&self, dwarf: &Dwarf<BinaryDataReader>) -> Result<Lines, gimli::Error> {
        let inc_line_program = match self.unit.line_program {
            Some(ref line_prog) => line_prog,
            None => return Ok(Lines::empty()),
        };

        let mut sequences = Vec::new();
        let mut rows = inc_line_program.clone().rows();
        let mut lines = Vec::new();

        let mut seq_start_addr = 0;
        let mut seq_prev_addr = 0;

        while let Some((_, row)) = rows.next_row()? {
            let address = row.address();

            if row.end_sequence() {
                if seq_start_addr != 0 && !lines.is_empty() {
                    // FIXME lines should be sorted by address I think but I'm not sure. If not I
                    //       should sort them here.
                    sequences.push(Sequence {
                        range: seq_start_addr..address,
                        lines: std::mem::replace(&mut lines, Vec::new()).into_boxed_slice(),
                    });
                } else {
                    // FIXME I'm not sure why it's not okay for the start address to be 0 (???)
                    //       It doesn't SEEM valid anyway.
                    lines.clear();
                }
            }

            let file = row.file_index() as usize;
            let line = row.line().unwrap_or(0) as u32;

            if !lines.is_empty() {
                if seq_prev_addr == address {
                    let last_line = lines.last_mut().unwrap();
                    last_line.file = file as usize;
                    last_line.line = line;
                    continue;
                } else {
                    seq_prev_addr = address;
                }
            } else {
                seq_start_addr = address;
                seq_prev_addr = address;
            }

            lines.push(Line {
                addr: address,
                file,
                line,
            });
        }

        sequences.sort_by_key(|seq| seq.range.start);

        let mut files = Vec::new();
        let header = inc_line_program.header();
        let mut idx = 0;
        while let Some(file) = header.file(idx) {
            let mut path = PathBuf::new();

            if let Some(directory) = file.directory(&header) {
                let directory_raw = dwarf.attr_string(&self.unit, directory)?;

                if let Ok(directory) = std::str::from_utf8(directory_raw.bytes()) {
                    path.push(directory);
                }
            }

            let file_path_raw = dwarf.attr_string(&self.unit, file.path_name())?;
            if let Ok(file_path) = std::str::from_utf8(file_path_raw.bytes()) {
                path.push(file_path);
                files.push(path);
            }

            idx += 1;
        }

        Ok(Lines {
            sequences: sequences.into_boxed_slice(),
            files: files.into_boxed_slice(),
        })
    }
}

struct Lines {
    sequences: Box<[Sequence]>,
    files: Box<[PathBuf]>,
}

impl Lines {
    fn empty() -> Lines {
        Lines {
            sequences: Box::new([] as [Sequence; 0]),
            files: Box::new([] as [PathBuf; 0]),
        }
    }

    fn lines_for_addr(&self, addr: u64) -> Option<impl '_ + Iterator<Item = (&Path, u32)>> {
        let map_line = move |line: &Line| (self.files[line.file].as_path(), line.line);

        let sequence = self
            .sequences
            .binary_search_by(|probe| util::cmp_range_to_idx(&probe.range, addr))
            .ok()
            .and_then(|seq_idx| self.sequences.get(seq_idx))?;

        if let Ok(idx) = sequence
            .lines
            .binary_search_by(|probe| probe.addr.cmp(&addr))
        {
            let mut range = idx..(idx + 1);

            // Find the first line with the address
            while range.start > 0 && sequence.lines[range.start - 1].addr == addr {
                range.start -= 1;
            }

            // Find the final line with the address.
            while range.end < sequence.lines.len() && sequence.lines[range.end].addr == addr {
                range.end += 1;
            }

            Some((&sequence.lines[range] as &[Line]).iter().map(map_line))
        } else {
            None
        }
    }
}

/// A contiguous sequence of bytes and their associated lines.
/// More than one line can be mapped to a single (or a block of) instruction(s).
struct Sequence {
    range: Range<u64>,
    lines: Box<[Line]>,
}

struct Line {
    addr: u64,
    file: usize,
    line: u32,
}

struct NameChain {
    names: Vec<(BinaryDataReader, isize)>,
    length: usize,
    depth: isize,
}

impl NameChain {
    fn new() -> NameChain {
        NameChain {
            names: Vec::with_capacity(32),
            length: 0,
            depth: 0,
        }
    }

    fn set_depth(&mut self, d: isize) {
        if d <= self.depth {
            self.depth = d;
            let mut remove = 0;
            for &(_, d) in self.names.iter().rev() {
                if d < self.depth {
                    break;
                } else {
                    remove += 1;
                }
            }

            for (name, _) in self.names.drain((self.names.len() - remove)..) {
                self.length -= name.len();
            }
        } else {
            self.depth = d;
        }
    }

    fn push(&mut self, name: BinaryDataReader) {
        self.length += name.len();
        self.names.push((name, self.depth));
    }

    fn combine(&self, separator: &str) -> String {
        let mut ret = String::new();
        if self.names.is_empty() {
            return ret;
        }
        let reserve = self.length + (separator.len() * (self.names.len() - 1));
        ret.reserve(reserve);

        let mut first = true;
        for (name, _) in self.names.iter() {
            if first {
                first = false;
            } else {
                ret.push_str(separator);
            }
            ret.push_str(if let Ok(n) = std::str::from_utf8(name.bytes()) {
                n
            } else {
                "?"
            });
        }
        assert_eq!(reserve, ret.len());

        ret
    }

    #[allow(dead_code)]
    fn clear(&mut self) {
        self.names.clear();
        self.length = 0;
        self.depth = 0;
    }
}
