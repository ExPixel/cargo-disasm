use crate::binary::BinaryData;
use ::pdb::PDB;

pub struct PDBInfo {
    pdb: PDB<'static, BinaryData>,
}
