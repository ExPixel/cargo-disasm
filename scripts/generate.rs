use std::error::Error as StdError;
use std::process::{self, Command, Stdio};

pub fn main() {
    if let Err(err) = generate_capstone_detail_types() {
        eprintln!("error: {}", err);

        let mut prev_source = &err as &dyn StdError;
        while let Some(source) = prev_source.source() {
            eprintln!("  caused by: {}", source);
            prev_source = source;
        }

        process::exit(-1);
    }

    process::exit(0);
}

fn generate_capstone_detail_types() -> Result<(), Error> {
    const DEST: &str = "capstone/src/arch/generated.rs";
    const HEADER: &str = "capstone/clib/include/capstone/capstone.h";

    const TYPES: &[&str] = &[
        "cs_x86",
        "cs_arm64",
        "cs_arm",
        "cs_m68k",
        "cs_mips",
        "cs_ppc",
        "cs_sparc",
        "cs_sysz",
        "cs_xcore",
        "cs_tms320c64x",
        "cs_m680x",
        "cs_evm",
        "cs_mos65xx",
    ];

    println!("generating capstone bindings...");

    let mut cmd = Command::new("bindgen");
    cmd.stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .args(&[
            "--verbose",
            "--use-core",
            "--size_t-is-usize",
            "--no-prepend-enum-name",
            "--no-doc-comments",
        ])
        .args(&["--ctypes-prefix", "libc"])
        .args(&["--rust-target", "1.36"]);
    for ty in TYPES.iter().copied() {
        cmd.args(&["--whitelist-type", ty]);
    }

    cmd.arg("-o").arg(DEST).arg(HEADER);

    println!("executing {:?}", cmd);

    let output = cmd
        .output()
        .map_err(|err| Error::with_cause("failed to run bindgen command", err))?;

    if !output.status.success() {
        return if let Some(code) = output.status.code() {
            Err(Error::with_msg(format!(
                "bindgen exited with error code {}",
                code
            )))
        } else {
            Err(Error::with_msg(format!(
                "bindgen exited with an unknown error code",
            )))
        };
    }

    Ok(())
}

#[derive(Debug)]
struct Error {
    message: String,
    cause: Option<Box<dyn StdError>>,
}

impl Error {
    pub fn with_msg(message: impl Into<String>) -> Self {
        Error {
            message: message.into(),
            cause: None,
        }
    }

    pub fn with_cause(message: impl Into<String>, cause: impl StdError + 'static) -> Self {
        Error {
            message: message.into(),
            cause: Some(Box::new(cause)),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.cause.as_deref()
    }
}
