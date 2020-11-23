use crate::disasm::binary::BinaryData;
use crate::disasm::symbol::{Symbol, SymbolSource};
use ::pdb::{AddressMap, FallibleIterator as _, ImageSectionHeader, ModuleInfo, SymbolData, PDB};
use anyhow::Context as _;

pub struct PDBInfo {
    pdb: PDB<'static, BinaryData>,
}

impl PDBInfo {
    pub fn new(data: BinaryData) -> anyhow::Result<PDBInfo> {
        PDB::open(data)
            .map(PDBInfo::with_pdb)
            .context("error while opening PDB")
    }

    fn with_pdb(pdb: PDB<'static, BinaryData>) -> Self {
        PDBInfo { pdb }
    }

    pub fn load_symbols(
        &mut self,
        image_base: u64,
        symbols: &mut Vec<Symbol>,
    ) -> anyhow::Result<()> {
        let sections = if let Some(sections) = self
            .pdb
            .sections()
            .context("error while reading PDB sections")?
        {
            sections
        } else {
            log::warn!("no sections defined in PDB");
            return Ok(());
        };

        let address_map = self
            .pdb
            .address_map()
            .context("error while reading PDB address map")?;

        let debug_information = self
            .pdb
            .debug_information()
            .context("error while getting PDB debug information")?;
        let mut modules_iter = debug_information
            .modules()
            .context("error while getting PDB modules")?;

        while let Some(module) = modules_iter
            .next()
            .context("error while reading PDB module")?
        {
            if let Some(module_info) = self
                .pdb
                .module_info(&module)
                .context("error while getting PDB module info")?
            {
                Self::load_symbols_from_module(
                    module_info,
                    &sections,
                    &address_map,
                    image_base,
                    symbols,
                )
                .context("error while loading symbols from PDB module")?
            }
        }
        Ok(())
    }

    fn load_symbols_from_module<'s>(
        module: ModuleInfo<'s>,
        sections: &[ImageSectionHeader],
        address_map: &AddressMap,

        image_base: u64,
        symbols: &mut Vec<Symbol>,
    ) -> anyhow::Result<()> {
        let mut symbol_iter = module.symbols()?;
        while let Some(symbol) = symbol_iter.next()? {
            // FIXME for now we just ignore symbol parse failures. The library is not complete
            //       and returns errors for unsupported symbol types.
            let data = match symbol.parse() {
                Ok(data) => data,
                Err(_err) => continue,
            };

            if let SymbolData::Procedure(procedure) = data {
                if procedure.offset.section == 0 {
                    continue;
                }

                let sym_offset =
                    if let Some(section) = sections.get(procedure.offset.section as usize - 1) {
                        section.pointer_to_raw_data as usize + procedure.offset.offset as usize
                    } else {
                        continue;
                    };

                let rva = procedure.offset.to_rva(&address_map).unwrap_or_default();

                // FIXME I'm not sure if I should actually be adding image_base to these. It makes
                // the addresses look like actual user process addresses but maybe it's not
                // necessary for disassembly.
                let sym_address = rva.0 as u64 + image_base;

                symbols.push(Symbol::new_unmangled(
                    procedure.name.to_string().into_owned(),
                    sym_address,
                    sym_offset,
                    procedure.len as usize,
                    SymbolSource::Pdb,
                ));
            }
        }
        Ok(())
    }
}
