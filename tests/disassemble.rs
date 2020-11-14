use std::error::Error;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus};

#[test]
pub fn disassemble() -> Result<(), Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_project_dir = manifest_dir.join("assets").join("pow");

    let build_project = cargo_build(&manifest_dir)?;
    assert!(build_project.success());

    let build_test_project = cargo_build(&test_project_dir)?;
    assert!(build_test_project.success());

    let disasm_test_project = cargo_disasm(&test_project_dir, "pow::my_pow")?;
    // FIXME ignore windows for now.
    if !cfg!(target_os = "windows") {
        assert!(disasm_test_project.success());
    }

    let disasm_current_project = cargo_disasm(&manifest_dir, "cargo_disasm::main")?;
    // FIXME ignore windows for now.
    if !cfg!(target_os = "windows") {
        assert!(disasm_current_project.success());
    }

    Ok(())
}

fn cargo_disasm<P, S>(disasm_dir: P, symbol: S) -> Result<ExitStatus, Box<dyn Error>>
where
    P: AsRef<Path>,
    S: AsRef<OsStr>,
{
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut disasm_exec_name = String::from("cargo-disasm");
    disasm_exec_name.push_str(std::env::consts::EXE_SUFFIX);
    let disasm_exec = manifest_dir
        .join("target")
        .join("debug")
        .join(&disasm_exec_name);
    let mut disasm_command = Command::new(disasm_exec);
    disasm_command.current_dir(disasm_dir);
    disasm_command.args(&[OsStr::new("-vvv"), symbol.as_ref()]);
    disasm_command.status().map_err(|err| err.into())
}

fn cargo_build<P: AsRef<Path>>(directory: P) -> Result<ExitStatus, Box<dyn Error>> {
    let mut build_command = Command::new("cargo");
    build_command.current_dir(directory.as_ref());
    build_command.args(&["build"]);
    build_command.status().map_err(|err| err.into())
}
