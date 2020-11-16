use super::{Arch, Binary, BinaryData, Bits, Endian, DWARF_SECTIONS};
use crate::disasm::dwarf::DwarfInfo;
use crate::disasm::pdb::PDBInfo;
use crate::disasm::symbol::{Symbol, SymbolLang, SymbolSource, SymbolType};

use anyhow::Context as _;
use goblin::pe::PE;
use std::path::{Path, PathBuf};

pub fn load_arch_info(binary: &mut Binary, pe: &PE) -> anyhow::Result<()> {
    log::debug!("object type   = PE/COFF");

    binary.bits = if pe.is_64 { Bits::Bits64 } else { Bits::Bits32 };
    binary.endian = Endian::Little;
    binary.arch = Arch::from_coff_machine(pe.header.coff_header.machine);

    log::debug!("object bits   = {}", binary.bits);
    log::debug!("object endian = {}", binary.endian);
    log::debug!("object arch   = {}", binary.arch);

    Ok(())
}

pub fn load_symbols(pe: &PE, data: &BinaryData, symbols: &mut Vec<Symbol>) -> anyhow::Result<()> {
    use goblin::pe;

    #[rustfmt::skip]
        let symtab = pe.header.coff_header.symbols(&*data)
            .context("error while loading COFF header symbol table")?;

    // There are no symbols in here >:(
    if symtab.get(0).is_none() {
        log::debug!("no symbols in PE/COFF object");
        return Ok(());
    }

    let strtab = pe
        .header
        .coff_header
        .strings(&*data)
        .context("error while loading COFF header string table")?;
    let pe_symbols_index = symbols.len();

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

        symbols.push(Symbol::new(
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
    for symbol in &mut symbols[pe_symbols_index..] {
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

pub fn load_pdb(_pdb: BinaryData) -> anyhow::Result<Box<PDBInfo>> {
    todo!("load pdb");
}

pub fn load_dwarf(pe: &PE, endian: Endian, data: &BinaryData) -> anyhow::Result<Box<DwarfInfo>> {
    let endian = gimli::RunTimeEndian::from(endian);
    let loader = |section: gimli::SectionId| {
        section_by_name(pe, &data, section.name()).map(|d| gimli::EndianReader::new(d, endian))
    };
    let sup_loader =
        |_section: gimli::SectionId| Ok(gimli::EndianReader::new(data.slice(0..0), endian));
    Ok(Box::new(DwarfInfo::new(loader, sup_loader)?))
}

pub fn find_pdb_path(pe: &PE, executable_path: &Path) -> anyhow::Result<Option<PathBuf>> {
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
                .or_else(|| Some(Path::new(executable_path.file_stem()?)))
                .and_then(|p| Some(executable_path.parent()?.join(p)))
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
            let mut buf = PathBuf::from(executable_path.parent()?);
            let mut name = executable_path.file_stem()?.to_owned();
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

pub fn contains_dwarf(pe: &PE) -> bool {
    pe.sections
        .iter()
        .filter_map(|section| section.name().ok())
        .any(|name| DWARF_SECTIONS.contains(&name))
}

fn section_by_name(pe: &PE, data: &BinaryData, name: &str) -> anyhow::Result<BinaryData> {
    for section in pe.sections.iter() {
        if section
            .name()
            .context("error while getting PE section name")?
            == name
        {
            let start = section.pointer_to_raw_data as usize;
            let end = start + section.size_of_raw_data as usize;
            return Ok(data.slice(start..end));
        }
    }
    Ok(data.slice(0..0))
}
