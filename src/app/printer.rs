use disasm::{symbol::Symbol, Disassembly};
use std::error::Error;
use termcolor::{Color, ColorSpec, WriteColor};

pub fn print_disassembly(
    out: &mut dyn WriteColor,
    sym: &Symbol,
    dis: &Disassembly,
) -> Result<(), Box<dyn Error>> {
    let measure = disasm::display::measure(dis);

    let space_sm = Spacing(2);
    let space_lg = Spacing(4);

    let max_addr = measure.max_address_width_hex(); // addr length
    let max_mnem = measure.max_mnemonic_len(); // mnemonic length
    let max_oprn = measure.max_operands_len(); // operand length
    let max_comm = measure.max_comments_len(); // comment length

    let clr_norm = ColorSpec::new(); // normal color

    let mut clr_addr = ColorSpec::new(); // address color
    clr_addr.set_fg(Some(Color::Blue));

    let mut clr_mnem = ColorSpec::new(); // mnemonic color
    clr_mnem.set_fg(Some(Color::Green));
    clr_mnem.set_bold(true);

    let clr_oprn = ColorSpec::new(); // operands color
    let mut clr_oprn_sym = clr_oprn.clone(); // operands color (for jumps to symbols)
    clr_oprn_sym.set_fg(Some(Color::Cyan));
    clr_oprn_sym.set_italic(true);

    let mut clr_comm = ColorSpec::new(); // comment color
    clr_comm.set_fg(Some(Color::Yellow));

    out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)).set_bold(true))?;
    writeln!(out, "{}:", sym.name())?;
    out.set_color(&clr_norm)?;

    for line in dis.lines() {
        out.set_color(&clr_norm)?;
        write!(out, "{}", space_sm)?;

        out.set_color(&clr_addr)?;
        write!(out, "{:<1$x}", line.address(), max_addr)?;

        out.set_color(&clr_norm)?;
        write!(out, "{}", space_lg)?;

        out.set_color(&clr_mnem)?;
        write!(out, "{:<1$}", line.mnemonic(), max_mnem)?;

        out.set_color(&clr_norm)?;
        write!(out, "{}", space_sm)?;

        if line.is_symbolicated_jump() {
            out.set_color(&clr_oprn_sym)?;
        } else {
            out.set_color(&clr_oprn)?;
        }
        write!(out, "{:<1$}", line.operands(), max_oprn)?;

        out.set_color(&clr_norm)?;

        if !line.comments().is_empty() {
            write!(out, "{}", space_lg)?;
            out.set_color(&clr_comm)?;
            write!(out, "; {:<1$}", line.comments(), max_comm)?;
        }

        out.set_color(&clr_norm)?;
        writeln!(out)?;
    }

    Ok(())
}

pub struct Spacing(usize);

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
