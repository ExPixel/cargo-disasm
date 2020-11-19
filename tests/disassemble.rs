use std::error::Error;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

macro_rules! assert_cmd {
    ($name:expr, $output:expr) => {{
        let output = &$output;

        assert!(
            output.status.success(),
            "error occurred while running `{}`:\n\nstdout:\n{}\n\nstderr:\n{}",
            &$name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }};
}

fn compile_cargo_disasm() {
    static COMPILE_MAIN_PROJECT: std::sync::Once = std::sync::Once::new();

    COMPILE_MAIN_PROJECT.call_once(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let build_project = cargo_build(&manifest_dir).expect("failed to build cargo-disasm");
        assert_cmd!("build cargo-disasm", build_project);
    });
}

#[test]
pub fn disassemble_cargo_disasm() -> Result<(), Box<dyn Error>> {
    compile_cargo_disasm();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let disasm_current_project = cargo_disasm(&manifest_dir, "cargo_disasm::main")?;
    assert_cmd!("disasm cargo-disasm", disasm_current_project);

    Ok(())
}

#[test]
pub fn disassemble_test_project() -> Result<(), Box<dyn Error>> {
    compile_cargo_disasm();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_project_dir = manifest_dir.join("assets").join("pow");

    let build_test_project = cargo_build(&test_project_dir)?;
    assert_cmd!("build pow", build_test_project);

    let disasm_test_project = cargo_disasm(&test_project_dir, "pow::my_pow")?;
    assert_cmd!("disasm pow", disasm_test_project);

    Ok(())
}

#[test]
pub fn disasm_test_project_aarch64_pc_windows_msvc() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("aarch64-pc-windows-msvc")
}

#[test]
pub fn disasm_test_project_aarch64_unknown_linux_gnu() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("aarch64-unknown-linux-gnu")
}

#[test]
pub fn disasm_test_project_i686_pc_windows_msvc() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("i686-pc-windows-msvc")
}

#[test]
pub fn disasm_test_project_i686_unknown_linux_gnu() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("i686-unknown-linux-gnu")
}

#[test]
pub fn disasm_test_project_x86_64_apple_darwin() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("x86_64-apple-darwin")
}

#[test]
pub fn disasm_test_project_x86_64_pc_windows_gnu() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("x86_64-pc-windows-gnu")
}

#[test]
pub fn disasm_test_project_x86_64_pc_windows_msvc() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("x86_64-pc-windows-msvc")
}

#[test]
pub fn disasm_test_project_x86_64_unknown_linux_gnu() -> Result<(), Box<dyn Error>> {
    disassemble_test_project_plat("x86_64-unknown-linux-gnu")
}

pub fn disassemble_test_project_plat(platform: &str) -> Result<(), Box<dyn Error>> {
    compile_cargo_disasm();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_project_dir = manifest_dir.join("assets").join("pow");
    let mut test_project_bin = test_project_dir.join(platform).join("debug");
    if platform.contains("windows") {
        test_project_bin = test_project_bin.join("pow.exe");
    } else {
        test_project_bin = test_project_bin.join("pow");
    }

    let disasm_test_project =
        cargo_disasm_bin(&test_project_dir, &test_project_bin, "pow::my_pow")?;
    assert_cmd!(format!("disasm pow-{}", platform), disasm_test_project);

    Ok(())
}

fn cargo_disasm_bin<P, B, S>(
    disasm_dir: P,
    disasm_bin: B,
    symbol: S,
) -> Result<Output, Box<dyn Error>>
where
    P: AsRef<Path>,
    B: AsRef<OsStr>,
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
    disasm_command.args(&[OsStr::new("-vvv"), symbol.as_ref(), disasm_bin.as_ref()]);
    disasm_command.output().map_err(|err| err.into())
}

fn cargo_disasm<P, S>(disasm_dir: P, symbol: S) -> Result<Output, Box<dyn Error>>
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
    disasm_command.output().map_err(|err| err.into())
}

fn cargo_build<P: AsRef<Path>>(directory: P) -> Result<Output, Box<dyn Error>> {
    let mut build_command = Command::new("cargo");
    build_command.current_dir(directory.as_ref());
    build_command.args(&["build"]);
    build_command.output().map_err(|err| err.into())
}
