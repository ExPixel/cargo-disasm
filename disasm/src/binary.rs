use crate::error::Error;
use crate::{Symbol, SymbolLang, SymbolSource, SymbolType};
use gimli::{read::EndianReader, Dwarf, RunTimeEndian};
use goblin::{archive::Archive, elf::Elf, mach::Mach, pe::PE, Object};
use memmap::{Mmap, MmapOptions};
use pdb::PDB;
use std::convert::TryFrom as _;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::rc::Rc;

pub struct Binary {
    /// Shared binary data. This must be pinned because it is referred to
    data: BinaryData,

    /// DWARF debugging information that was found.
    dwarf: Option<Box<Dwarf<EndianReader<RunTimeEndian, BinaryData>>>>,

    /// PDB debugging information that was found.
    pdb: Option<Box<PDB<'static, BinaryData>>>,

    arch: Arch,
    endian: Endian,
    bits: Bits,
}

impl Binary {
    pub fn new<SymFn>(data: BinaryData, sym_fn: SymFn) -> Result<Binary, Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        let mut binary = Binary {
            data,
            dwarf: None,
            pdb: None,

            arch: Arch::Unknown,
            endian: Endian::Unknown,
            bits: Bits::Unknown,
        };
        binary.parse_object(sym_fn).map(|_| binary)
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

    fn parse_object<SymFn>(&mut self, sym_fn: SymFn) -> Result<(), Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        let data = self.data.clone();
        match Object::parse(&data)
            .map_err(|err| Error::new("failed to parse object", Box::new(err)))?
        {
            Object::Elf(elf) => self.parse_elf_object(&elf, sym_fn),
            Object::PE(pe) => self.parse_pe_object(&pe, sym_fn),
            Object::Mach(mach) => self.parse_mach_object(&mach, sym_fn),
            Object::Archive(archive) => self.parse_archive_object(&archive, sym_fn),
            Object::Unknown(magic) => Err(Error::msg(format!(
                "failed to parse object with magic value 0x{:X}",
                magic
            ))),
        }
    }

    fn parse_elf_object<SymFn>(&mut self, elf: &Elf, mut sym_fn: SymFn) -> Result<(), Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        use goblin::elf::header;

        self.bits = Bits::from_elf_class(elf.header.e_ident[header::EI_CLASS]);
        self.endian = Endian::from(
            elf.header
                .endianness()
                .map_err(|e| Error::new("failed to identify ELF endianness", Box::new(e)))?,
        );
        self.arch = Arch::from_elf_machine(elf.header.e_machine);

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
                .map_err(|e| Error::new("failed to get symbol name", Box::new(e)))?
            {
                name
            } else {
                continue;
            };

            let (section_offset, section_addr) = {
                let sym_section = elf.section_headers.get(sym.st_shndx).ok_or_else(|| {
                    Error::msg(format!(
                        "no matching section header for {} (header-idx: {})",
                        sym_name, sym.st_shndx
                    ))
                })?;
                (sym_section.sh_offset, sym_section.sh_addr)
            };

            // FIXME clamp values to section bounds.
            // FIXME This works for executable and shared objects that use st_value as a virtual
            // address to the symbol, but I also want to handle relocatable files, in which case
            // st_value would hold a section offset for the symbol.
            let sym_addr = sym.st_value;
            let sym_offset = (sym_addr - section_addr) + section_offset;

            let symbol = Symbol::new(
                sym_name,
                sym_addr,
                sym_offset as usize,
                sym.st_size as usize,
                SymbolType::Function,
                SymbolSource::Object,
                SymbolLang::Unknown,
            );

            sym_fn(symbol);
        }

        Ok(())
    }

    fn parse_mach_object<SymFn>(&mut self, mach: &Mach, sym_fn: SymFn) -> Result<(), Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        Err(Error::msg("mach objects are not currently supported"))
    }

    fn parse_pe_object<SymFn>(&mut self, pe: &PE, sym_fn: SymFn) -> Result<(), Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        Err(Error::msg("PE objects are not currently supported"))
    }

    fn parse_archive_object<SymFn>(&mut self, archive: &Archive, sym_fn: SymFn) -> Result<(), Error>
    where
        SymFn: for<'a> FnMut(Symbol<'a>),
    {
        Err(Error::msg("archive objects are not currently supported"))
    }
}

/// Reference counted and memory mapped binary data.
#[derive(Clone)]
pub struct BinaryData {
    /// The current offset of the binary data that is being read.
    offset: usize,

    inner: Rc<Mmap>,
}

impl BinaryData {
    /// Loads binary data from a file.
    pub fn from_file(file: &File) -> io::Result<Self> {
        unsafe {
            MmapOptions::new().map(&file).map(|m| BinaryData {
                offset: 0,
                inner: Rc::new(m),
            })
        }
    }
}

impl std::fmt::Debug for BinaryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinaryData")
            .field("len", &self.inner.len())
            .finish()
    }
}

impl std::ops::Deref for BinaryData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl Read for BinaryData {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut slice: &[u8] = &*self.inner;

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
