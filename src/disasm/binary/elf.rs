use super::{Arch, Binary, BinaryData, Bits, Endian, DWARF_SECTIONS};
use crate::disasm::dwarf::DwarfInfo;
use crate::disasm::symbol::{Symbol, SymbolLang, SymbolSource, SymbolType};
use crate::util;
use anyhow::Context as _;
use goblin::elf::Elf;

pub fn load_arch_info(binary: &mut Binary, elf: &Elf) -> anyhow::Result<()> {
    use goblin::elf::header;

    log::debug!("object type   = ELF");

    binary.bits = Bits::from_elf_class(elf.header.e_ident[header::EI_CLASS]);
    binary.endian = Endian::from(
        elf.header
            .endianness()
            .context("failed to identify ELF endianness")?,
    );
    binary.arch = Arch::from_elf_machine(elf.header.e_machine);

    log::debug!("object bits   = {}", binary.bits);
    log::debug!("object endian = {}", binary.endian);
    log::debug!("object arch   = {}", binary.arch);

    Ok(())
}

pub fn load_symbols(elf: &Elf, symbols: &mut Vec<Symbol>) -> anyhow::Result<()> {
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

        symbols.push(Symbol::new(
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

pub fn load_dwarf(elf: &Elf, binary: &Binary) -> anyhow::Result<Box<DwarfInfo>> {
    use gimli::EndianReader;
    use gimli::RunTimeEndian;

    let endian = RunTimeEndian::from(binary.endian);

    let loader = |section: gimli::SectionId| {
        section_by_name(elf, section.name(), &binary.data).map(|d| EndianReader::new(d, endian))
    };

    let sup_loader =
        |_section: gimli::SectionId| Ok(EndianReader::new(binary.data.slice(0..0), endian));

    Ok(Box::new(DwarfInfo::new(loader, sup_loader)?))
}

pub fn load_dwarf_symbols(
    elf: &Elf,
    dwarf: &DwarfInfo,
    symbols: &mut Vec<Symbol>,
) -> anyhow::Result<()> {
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

    let symbols_count_before = symbols.len();
    let load_symbols_timer = std::time::Instant::now();
    dwarf.load_symbols(symbols, addr_to_offset)?;

    Ok(())
}

pub fn contains_dwarf(elf: &Elf) -> bool {
    elf.section_headers
        .iter()
        .filter_map(|header| {
            // ugh
            elf.shdr_strtab
                .get(header.sh_name)
                .transpose()
                .ok()
                .flatten()
        })
        .any(|name| DWARF_SECTIONS.contains(&name))
}

fn section_by_name(elf: &Elf, name: &str, data: &BinaryData) -> anyhow::Result<BinaryData> {
    for section in elf.section_headers.iter() {
        let section_name = elf
            .shdr_strtab
            .get(section.sh_name)
            .transpose()
            .context("failed to retrieve ELF section name")?;
        if section_name == Some(name) {
            let start = section.sh_offset as usize;
            let end = start + section.sh_size as usize;
            return Ok(data.slice(start..end));
        }
    }
    Ok(data.slice(0..0))
}
