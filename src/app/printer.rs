use crate::disasm::strmatch::Tokenizer;
use crate::disasm::{self, symbol::Symbol, Disassembly};
use termcolor::{Color, ColorSpec, WriteColor};

const MAX_OPERAND_LEN: usize = 72;

pub fn print_disassembly(
    out: &mut dyn WriteColor,
    sym: &Symbol,
    dis: &Disassembly,
    opt: DisasmOptions,
) -> anyhow::Result<()> {
    let measure = disasm::display::measure(dis);

    let space_sm = Spacing(2);
    let space_lg = Spacing(4);

    let max_addr = measure.max_address_width_hex(); // addr length
    let max_mnem = measure.max_mnemonic_len(); // mnemonic length
    let mut max_oprn = measure.max_operands_len(); // operand length
    let max_comm = measure.max_comments_len(); // comment length
    let max_bytes = measure.max_bytes_width_hex(1); // bytes length

    let addr_indent = space_sm;
    let bytes_indent = addr_indent + max_addr + space_lg;
    let mnem_indent = bytes_indent
        + if opt.show_bytes {
            space_sm + max_bytes // spacing comes after
        } else {
            Spacing(0)
        };
    let oprn_indent = mnem_indent + max_mnem + space_sm;
    let source_indent = bytes_indent;

    if max_oprn > MAX_OPERAND_LEN {
        max_oprn = MAX_OPERAND_LEN;
    }

    let clr_norm = ColorSpec::new(); // normal color

    let mut clr_addr = ColorSpec::new(); // address color
    clr_addr.set_fg(Some(Color::Blue));

    let mut clr_bytes = ColorSpec::new();
    clr_bytes.set_fg(Some(Color::Yellow));

    let mut clr_source = ColorSpec::new(); // mnemonic color
    clr_source.set_fg(Some(Color::Magenta));
    clr_source.set_bold(true);

    let mut clr_mnem = ColorSpec::new(); // mnemonic color
    clr_mnem.set_fg(Some(Color::Green));
    clr_mnem.set_bold(true);

    let clr_oprn = ColorSpec::new(); // operands color
    let mut clr_oprn_sym = clr_oprn.clone(); // operands color (for jumps to symbols)
    clr_oprn_sym.set_fg(Some(Color::Cyan));

    let mut clr_comm = ColorSpec::new(); // comment color
    clr_comm.set_italic(true);
    clr_comm.set_fg(Some(Color::Yellow));

    out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)).set_bold(true))?;
    writeln!(out, "{}:", sym.name())?;
    out.set_color(&clr_norm)?;

    for line in dis.lines() {
        if opt.show_source {
            for source_line in line.source_lines() {
                out.set_color(&clr_source)?;
                writeln!(out, "{}{}", source_indent, source_line)?;
            }
        }

        out.set_color(&clr_norm)?;
        write!(out, "{}", space_sm)?;

        out.set_color(&clr_addr)?;
        write!(out, "{:<1$x}", line.address(), max_addr)?;

        out.set_color(&clr_norm)?;
        write!(out, "{}", space_lg)?;

        if opt.show_bytes {
            out.set_color(&clr_bytes)?;
            write!(out, "{:>1$}", Hex(line.bytes()), max_bytes)?;

            out.set_color(&clr_norm)?;
            write!(out, "{}", space_sm)?;
        }

        out.set_color(&clr_mnem)?;
        write!(out, "{:<1$}", line.mnemonic(), max_mnem)?;

        out.set_color(&clr_norm)?;
        write!(out, "{}", space_sm)?;

        let oprn_color = if line.is_symbolicated_jump() {
            clr_oprn_sym
                .set_italic(line.jump().is_external())
                .set_bold(line.jump().is_internal());
            &clr_oprn_sym
        } else {
            &clr_oprn
        };
        out.set_color(oprn_color)?;

        let mut operands = WordWrapped::new(line.operands(), max_oprn);
        let mut has_more_operands = false;
        let mut operand_chars_printed = 0;
        while let Some(operand) = operands.next() {
            if let WrappedStr::Str(token) = operand {
                operand_chars_printed += token.len();
                write!(out, "{}", token)?;
            } else {
                has_more_operands = true;
                break;
            }
        }

        // Write the comment after the first line of the operands:
        if !line.comments().is_empty() {
            out.set_color(&clr_norm)?;
            write!(
                out,
                "{}",
                Spacing(space_lg.0 + (max_oprn - operand_chars_printed))
            )?;
            out.set_color(&clr_comm)?;
            write!(out, "; {:<1$}", line.comments(), max_comm)?;
        }

        // Write the remaining lines of the operands if there are any:
        if has_more_operands {
            out.set_color(&clr_norm)?;
            writeln!(out)?;
            write!(out, "{}", oprn_indent)?;
            let mut in_oprn_color = false;
            for w in operands {
                match w {
                    WrappedStr::Str(s) => {
                        if !in_oprn_color {
                            out.set_color(oprn_color)?;
                            in_oprn_color = true;
                        }
                        write!(out, "{}", s)?;
                    }

                    WrappedStr::Break => {
                        if in_oprn_color {
                            out.set_color(&clr_norm)?;
                            in_oprn_color = false;
                        }
                        writeln!(out)?;
                        write!(out, "{}", oprn_indent)?;
                    }
                }
            }
        } else {
            out.set_color(&clr_norm)?;
        }
        writeln!(out)?;
    }

    Ok(())
}

pub struct Hex<'b>(&'b [u8]);

impl std::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const LOWER: [u8; 16] = [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd',
            b'e', b'f',
        ];
        const UPPER: [u8; 16] = [
            b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D',
            b'E', b'F',
        ];

        let output_width = if self.0.is_empty() {
            0
        } else {
            (self.0.len() as usize * 2) + (self.0.len() as usize - 1)
        };
        let mut buffer = String::with_capacity(output_width);

        for &byte in self.0 {
            if !buffer.is_empty() {
                buffer.push(' ');
            }

            if f.alternate() {
                buffer.push(UPPER[(byte >> 4) as usize] as char);
                buffer.push(UPPER[(byte & 15) as usize] as char);
            } else {
                buffer.push(LOWER[(byte >> 4) as usize] as char);
                buffer.push(LOWER[(byte & 15) as usize] as char);
            }
        }

        f.pad(&buffer)
    }
}

#[derive(Copy, Clone)]
pub struct Spacing(usize);

impl std::ops::Add<Self> for Spacing {
    type Output = Self;

    fn add(self, other: Spacing) -> Self {
        Spacing(self.0 + other.0)
    }
}

impl std::ops::AddAssign<Self> for Spacing {
    fn add_assign(&mut self, other: Spacing) {
        *self = *self + other;
    }
}

impl std::ops::Sub<Self> for Spacing {
    type Output = Self;

    fn sub(self, other: Spacing) -> Self {
        Spacing(self.0 - other.0)
    }
}

impl std::ops::SubAssign<Self> for Spacing {
    fn sub_assign(&mut self, other: Spacing) {
        *self = *self - other;
    }
}

impl std::ops::Add<usize> for Spacing {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        Spacing(self.0 + other)
    }
}

impl std::ops::AddAssign<usize> for Spacing {
    fn add_assign(&mut self, other: usize) {
        *self = *self + other;
    }
}

impl std::ops::Sub<usize> for Spacing {
    type Output = Self;

    fn sub(self, other: usize) -> Self {
        Spacing(self.0 - other)
    }
}

impl std::ops::SubAssign<usize> for Spacing {
    fn sub_assign(&mut self, other: usize) {
        *self = *self - other;
    }
}

impl std::fmt::Display for Spacing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut spacing = self.0;

        loop {
            if spacing >= 8 {
                f.write_str("        ")?;
                spacing -= 8;
            } else if spacing >= 4 {
                f.write_str("    ")?;
                spacing -= 4;
            } else if spacing >= 2 {
                f.write_str("  ")?;
                spacing -= 2;
            } else {
                f.write_str(" ")?;
                spacing -= 1;
            }

            if spacing == 0 {
                return Ok(());
            }
        }
    }
}

pub struct WordWrapped<'s> {
    max_len: usize,
    cur_len: usize,
    tokens: Tokenizer<'s>,
    pending: Option<&'s str>,
}

impl<'s> WordWrapped<'s> {
    pub fn new(string: &'s str, max_len: usize) -> WordWrapped<'s> {
        WordWrapped {
            max_len,
            cur_len: 0,
            tokens: Tokenizer::no_whitespace_normalize(string),
            pending: None,
        }
    }
}

impl<'s> Iterator for WordWrapped<'s> {
    type Item = WrappedStr<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pending) = self.pending.take() {
            self.cur_len += pending.len();
            return Some(WrappedStr::Str(pending));
        }

        let next_token = self.tokens.next()?;
        if self.cur_len + next_token.len() > self.max_len {
            self.cur_len = 0;
            self.pending = Some(next_token);
            Some(WrappedStr::Break)
        } else {
            self.cur_len += next_token.len();
            Some(WrappedStr::Str(next_token))
        }
    }
}

pub enum WrappedStr<'s> {
    Str(&'s str),
    Break,
}

#[derive(Copy, Clone)]
pub struct DisasmOptions {
    pub show_bytes: bool,
    pub show_source: bool,
}
