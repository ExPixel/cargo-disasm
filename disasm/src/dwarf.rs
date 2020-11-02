use crate::binary::BinaryData;
use crate::error::Error;
use gimli::{read::EndianReader, Dwarf, RunTimeEndian};
use std::ops::Range;

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
}

impl LazyCompilationUnit {
    pub fn new(
        unit: gimli::Unit<BinaryDataReader>,
        lang: Option<gimli::DwLang>,
    ) -> LazyCompilationUnit {
        LazyCompilationUnit { unit, lang }
    }
}
