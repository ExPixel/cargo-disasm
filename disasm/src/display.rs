use super::Disassembly;

pub fn measure(disassembly: &Disassembly) -> DisasmDisplayMeasure {
    let mut measure = DisasmDisplayMeasure::default();

    for line in disassembly.lines() {
        measure.max_address = std::cmp::max(measure.max_address, line.address());
        measure.max_mnemonic_len =
            std::cmp::max(measure.max_mnemonic_len, line.mnemonic().len() as u16);
        measure.max_operands_len =
            std::cmp::max(measure.max_operands_len, line.mnemonic().len() as u16);
        measure.max_comments_len =
            std::cmp::max(measure.max_comments_len, line.comments().len() as u16);
    }

    measure
}

/// Measurements for the table.
#[derive(Default)]
pub struct DisasmDisplayMeasure {
    /// The maximum address that needs to be displayed in the table.
    max_address: u64,
    /// The maximum length of a mnemonic that has to be displayed in the table.
    max_mnemonic_len: u16,
    /// The maximum length of an operand that has to be displayed in the table.
    max_operands_len: u16,
    /// The maximum length of comments that has to be displayed in the table.
    max_comments_len: u16,
}

impl DisasmDisplayMeasure {
    /// Returns the maximum address width in hexidecimal characters.
    /// This value should be cached somewhere instead of calling this
    /// function multiple times if possible.
    #[inline]
    pub fn max_address_width_hex(&self) -> usize {
        ((64 - self.max_address.leading_zeros()) as f64 / 4.0).ceil() as usize
    }

    #[inline]
    pub fn max_mnemonic_len(&self) -> usize {
        self.max_mnemonic_len as usize
    }

    #[inline]
    pub fn max_operands_len(&self) -> usize {
        self.max_operands_len as usize
    }

    #[inline]
    pub fn max_comments_len(&self) -> usize {
        self.max_comments_len as usize
    }
}
