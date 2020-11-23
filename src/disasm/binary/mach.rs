use super::{Arch, Binary, BinaryData, Bits, Endian, DWARF_SECTIONS};
use crate::disasm::dwarf::DwarfInfo;
use crate::disasm::symbol::{Symbol, SymbolSource};
use crate::util;
use anyhow::Context as _;
use goblin::mach::segment::Section;
use goblin::mach::{Mach, MachO};
use std::path::{Path, PathBuf};

pub fn load_arch_info(binary: &mut Binary, mach: &MachO) -> anyhow::Result<()> {
    log::debug!("object type   = Mach-O");

    binary.bits = if mach.is_64 {
        Bits::Bits64
    } else {
        Bits::Bits32
    };
    binary.endian = if mach.little_endian {
        Endian::Little
    } else {
        Endian::Big
    };
    binary.arch = Arch::from_mach_cpu_types(mach.header.cputype, mach.header.cpusubtype);

    log::debug!("object bits   = {}", binary.bits);
    log::debug!("object endian = {}", binary.endian);
    log::debug!("object arch   = {}", binary.arch);

    Ok(())
}

pub fn load_symbols(
    mach: &MachO,
    sections: &[Section],
    symbols: &mut Vec<Symbol>,
) -> anyhow::Result<()> {
    use goblin::mach::symbols;

    // The starting index for Mach symbols in the `symbols` vector.
    let mach_symbols_idx = symbols.len();

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

        symbols.push(Symbol::new(
            sym_name,
            sym_addr,
            sym_offset as usize,
            0, // this is fixed later
            SymbolSource::Mach,
        ));
    }

    symbol_addresses.sort_unstable();
    symbol_addresses.dedup();

    // Figure out where symbols end by using the starting address of the next symbol.
    for symbol in &mut symbols[mach_symbols_idx..] {
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

pub fn load_dwarf(
    sections: &[Section],
    endian: Endian,
    data: &BinaryData,
) -> anyhow::Result<Option<Box<DwarfInfo>>> {
    if let dwarf @ Some(_) = load_dsym_dwarf(data)? {
        return Ok(dwarf);
    }

    if !contains_dwarf(sections) {
        return Ok(None);
    }

    let endian = gimli::RunTimeEndian::from(endian);
    let loader = |section: gimli::SectionId| {
        section_by_name(&sections, &data, section.name())
            .map(|d| gimli::EndianReader::new(d, endian))
    };
    let sup_loader =
        |_section: gimli::SectionId| Ok(gimli::EndianReader::new(data.slice(0..0), endian));
    Ok(Some(Box::new(DwarfInfo::new(loader, sup_loader)?)))
}

fn load_dsym_dwarf(data: &BinaryData) -> anyhow::Result<Option<Box<DwarfInfo>>> {
    let dsym_directory = if let Some(d) = find_dsym_directory(data.path()) {
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
        if let Some(fname) = data.path().file_name() {
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

    let data = BinaryData::from_path(&object_path).context("failed to load Mach-O DWARF binary")?;
    let mach = Mach::parse(&data)
        .with_context(|| format!("failed to parse Mach-O binary {}", object_path.display()))?;
    let mach = match mach {
        goblin::mach::Mach::Fat(multi) => multi
            .get(0)
            .context("failed to get first object from fat Mach binary")?,
        goblin::mach::Mach::Binary(obj) => obj,
    };

    let sections = load_sections(&mach)?;

    let endian = if mach.little_endian {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    let loader = |section: gimli::SectionId| {
        section_by_name(&sections, &data, section.name())
            .map(|d| gimli::EndianReader::new(d, endian))
    };
    let sup_loader =
        |_section: gimli::SectionId| Ok(gimli::EndianReader::new(data.slice(0..0), endian));
    let dwarf = Box::new(DwarfInfo::new(loader, sup_loader)?);

    Ok(Some(dwarf))
}

pub fn load_dwarf_symbols(
    dwarf: &DwarfInfo,
    sections: &[Section],
    symbols: &mut Vec<Symbol>,
) -> anyhow::Result<()> {
    let addr_to_offset = move |addr| {
        sections
            .binary_search_by(|probe| {
                util::cmp_range_to_idx(&(probe.addr..(probe.addr + probe.size as u64)), addr)
            })
            .ok()
            .map(|idx| (addr - sections[idx].addr) as usize + sections[idx].offset as usize)
    };
    dwarf.load_symbols(symbols, addr_to_offset)?;
    Ok(())
}

pub fn load_sections(mach: &MachO) -> anyhow::Result<Vec<Section>> {
    let mut sections: Vec<Section> = Vec::new();
    for segment in mach.segments.iter() {
        for s in segment.into_iter() {
            let (section, _) = s.context("error occured while getting Mach-O section")?;
            sections.push(section);
        }
    }
    Ok(sections)
}

/// Find the dSYM directory relative to an executable.
fn find_dsym_directory(executable_path: &Path) -> Option<PathBuf> {
    let executable_dir = executable_path.parent()?;
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

fn section_by_name(
    sections: &[Section],
    data: &BinaryData,
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

pub fn contains_dwarf(sections: &[Section]) -> bool {
    sections
        .iter()
        .filter_map(|section| section.name().ok())
        .any(|name| MACH_DWARF_SECTIONS.contains(&name) || DWARF_SECTIONS.contains(&name))
}

const MACH_TYPE_FUNC: u8 = 0x24;

/// Names used for detecting DWARF debug information in Mach object files.
const MACH_DWARF_SECTIONS: &[&str] = &[
    "__debug_abbrev",
    "__debug_addr",
    "__debug_info",
    "__debug_line",
    "__debug_line_str",
    "__debug_str",
    "__debug_str_offsets",
    "__debug_types",
    "__debug_loc",
    "__debug_loclists",
    "__debug_ranges",
    "__debug_rnglists",
];
