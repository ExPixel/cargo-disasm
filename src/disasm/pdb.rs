use crate::disasm::binary::BinaryData;
use ::pdb::PDB;

pub struct PDBInfo {
    pdb: PDB<'static, BinaryData>,
}
