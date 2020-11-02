use crate::binary::BinaryData;
use crate::error::Error;
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
    pub fn new<L, S>(loader: L, sup_loader: S) -> Result<DwarfInfo, Error>
    where
        L: Fn(gimli::SectionId) -> Result<BinaryDataReader, Error>,
        S: Fn(gimli::SectionId) -> Result<BinaryDataReader, Error>,
    {
        Ok(DwarfInfo {
            dwarf: gimli::Dwarf::load(loader, sup_loader)?,

            compilation_unit_ranges: Vec::new(),
            compilation_units: Vec::new(),
            compilation_units_initialized: false,
        })
    }

    /// This will load the compilation units and their addresses ranges
    /// if it has not been done already.
    pub fn ensure_compilation_units(&mut self) -> Result<(), Error> {
        if self.compilation_units_initialized {
            return Ok(());
        }
        self.compilation_units_initialized = true;

        Self::find_compilation_units(
            &self.dwarf,
            &mut self.compilation_units,
            &mut self.compilation_unit_ranges,
        )
        .map_err(|err| Error::new("error while finding compilation units", Box::new(err)))
    }

    #[cold]
    fn find_compilation_units(
        dwarf: &Dwarf<BinaryDataReader>,
        units: &mut Vec<LazyCompilationUnit>,
        ranges: &mut Vec<UnitRange>,
    ) -> Result<(), gimli::Error> {
        let mut unit_headers = dwarf.units();
        while let Some(unit_header) = unit_headers.next()? {
            let unit = if let Ok(unit) = dwarf.unit(unit_header) {
                unit
            } else {
                continue;
            };

            Self::add_compilation_unit(unit, dwarf, units, ranges)?;
        }

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
        self.lines.get_or_try_init(|| self.load_lines(dwarf))
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

    fn lines_for_addr(&self, addr: u64) -> Option<(&Path, u32)> {
        let sequence = self
            .sequences
            .binary_search_by(|probe| {
                if probe.range.start > addr {
                    std::cmp::Ordering::Greater
                } else if probe.range.end <= addr {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .and_then(|seq_idx| self.sequences.get(seq_idx))?;

        sequence
            .lines
            .binary_search_by(|probe| probe.addr.cmp(&addr))
            .ok()
            .and_then(|line_idx| sequence.lines.get(line_idx))
            .map(|line| (self.files[line.file].as_path(), line.line))
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
