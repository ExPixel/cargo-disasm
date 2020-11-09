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

    let disasm_test_project = cargo_disasm(&test_project_dir)?;

    // FIXME for now this is the only one I check.
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        assert!(disasm_test_project.success());
    }

    Ok(())
}

fn cargo_disasm<P: AsRef<Path>>(disasm_dir: P) -> Result<ExitStatus, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut disasm_exec_name = String::from("cargo-disasm");
    disasm_exec_name.push_str(std::env::consts::EXE_SUFFIX);
    let disasm_exec = manifest_dir
        .join("target")
        .join("debug")
        .join(&disasm_exec_name);
    let mut disasm_command = Command::new(disasm_exec);
    disasm_command.current_dir(disasm_dir);
    disasm_command.args(&[OsStr::new("-vvv"), OsStr::new("pow::my_pow")]);
    disasm_command.status().map_err(|err| err.into())
}

fn cargo_build<P: AsRef<Path>>(directory: P) -> Result<ExitStatus, Box<dyn Error>> {
    let mut build_command = Command::new("cargo");
    build_command.current_dir(directory.as_ref());
    build_command.args(&["build"]);
    build_command.status().map_err(|err| err.into())
}
