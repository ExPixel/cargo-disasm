use std::path::Path;

#[allow(unused_macros)]
macro_rules! warn {
    ($fmt:literal $(,$arg:expr)*) => {
        println!(concat!("cargo-warning=", $fmt) $(,$arg)*)
    };
}

const SOURCES_ENGINE: &[&str] = &[
    "clib/cs.c",
    "clib/MCInst.c",
    "clib/MCInstrDesc.c",
    "clib/MCRegisterInfo.c",
    "clib/SStream.c",
    "clib/utils.c",
];

const HEADERS_ENGINE: &[&str] = &[
    "clib/cs_priv.h",
    "clib/LEB128.h",
    "clib/MathExtras.h",
    "clib/MCDisassembler.h",
    "clib/MCFixedLenDisassembler.h",
    "clib/MCInst.h",
    "clib/MCInstrDesc.h",
    "clib/MCRegisterInfo.h",
    "clib/SStream.h",
    "clib/utils.h",
];

const HEADERS_COMMON: &[&str] = &[
    "clib/include/capstone/arm64.h",
    "clib/include/capstone/arm.h",
    "clib/include/capstone/capstone.h",
    "clib/include/capstone/evm.h",
    "clib/include/capstone/mips.h",
    "clib/include/capstone/ppc.h",
    "clib/include/capstone/x86.h",
    "clib/include/capstone/sparc.h",
    "clib/include/capstone/systemz.h",
    "clib/include/capstone/xcore.h",
    "clib/include/capstone/m68k.h",
    "clib/include/capstone/tms320c64x.h",
    "clib/include/capstone/m680x.h",
    "clib/include/capstone/mos65xx.h",
    "clib/include/capstone/platform.h",
];

fn main() {
    let mut build = cc::Build::new();

    build.flag_if_supported("-Wno-unused-parameter");
    build.flag_if_supported("-Wno-unused-variable");
    build.flag_if_supported("-Wno-sign-compare");
    build.flag_if_supported("-Wno-missing-field-initializers");

    build.files(SOURCES_ENGINE);
    build.include("clib"); // engine headers
    build.include("clib/include"); // common headers

    track(SOURCES_ENGINE);
    track(HEADERS_ENGINE);
    track(HEADERS_COMMON);

    if cfg!(feature = "diet") {
        build.define("CAPSTONE_DIET", None);
    }

    if cfg!(feature = "sys-dyn-mem") {
        build.define("CAPSTONE_USE_SYS_DYN_MEM", None);
    }

    if cfg!(feature = "x86-reduce") {
        build.define("CAPSTONE_X86_REDUCE", None);
    }

    if cfg!(feature = "x86-disable-att") {
        build.define("CAPSTONE_X86_ATT_DISABLE", None);
    }

    if cfg!(feature = "arm") {
        add_arm_support(&mut build);
    }

    if cfg!(feature = "aarch64") {
        add_arm64_support(&mut build);
    }

    if cfg!(feature = "mips") {
        add_mips_support(&mut build);
    }

    if cfg!(feature = "powerpc") {
        add_ppc_support(&mut build);
    }

    if cfg!(feature = "x86") {
        add_x86_support(&mut build);
    }

    if cfg!(feature = "sparc") {
        add_sparc_support(&mut build);
    }

    if cfg!(feature = "systemz") {
        add_sysz_support(&mut build);
    }

    if cfg!(feature = "xcore") {
        add_xcore_support(&mut build);
    }

    if cfg!(feature = "m68k") {
        add_m68k_support(&mut build);
    }

    if cfg!(feature = "tms320c64x") {
        add_tms320c64x_support(&mut build);
    }

    if cfg!(feature = "m680x") {
        add_m680x_support(&mut build);
    }

    if cfg!(feature = "evm") {
        add_evm_support(&mut build);
    }

    if cfg!(feature = "mos65xx") {
        add_mos65xx_support(&mut build);
    }

    build.file("./test_helper.c");
    track(&["./test_helper.c"]);

    build.compile("capstone");
}

fn add_arm_support(build: &mut cc::Build) {
    const SOURCES_ARM: &[&str] = &[
        "clib/arch/ARM/ARMDisassembler.c",
        "clib/arch/ARM/ARMInstPrinter.c",
        "clib/arch/ARM/ARMMapping.c",
        "clib/arch/ARM/ARMModule.c",
    ];

    const HEADERS_ARM: &[&str] = &[
        "clib/arch/ARM/ARMAddressingModes.h",
        "clib/arch/ARM/ARMBaseInfo.h",
        "clib/arch/ARM/ARMDisassembler.h",
        "clib/arch/ARM/ARMGenAsmWriter.inc",
        "clib/arch/ARM/ARMGenDisassemblerTables.inc",
        "clib/arch/ARM/ARMGenInstrInfo.inc",
        "clib/arch/ARM/ARMGenRegisterInfo.inc",
        "clib/arch/ARM/ARMGenSubtargetInfo.inc",
        "clib/arch/ARM/ARMInstPrinter.h",
        "clib/arch/ARM/ARMMapping.h",
        "clib/arch/ARM/ARMMappingInsn.inc",
        "clib/arch/ARM/ARMMappingInsnOp.inc",
    ];

    build.define("CAPSTONE_HAS_ARM", None);
    build.includes(uniq_dirs(HEADERS_ARM));
    build.files(SOURCES_ARM);

    track(SOURCES_ARM);
    track(HEADERS_ARM);
}

fn add_arm64_support(build: &mut cc::Build) {
    const SOURCES_ARM64: &[&str] = &[
        "clib/arch/AArch64/AArch64BaseInfo.c",
        "clib/arch/AArch64/AArch64Disassembler.c",
        "clib/arch/AArch64/AArch64InstPrinter.c",
        "clib/arch/AArch64/AArch64Mapping.c",
        "clib/arch/AArch64/AArch64Module.c",
    ];

    const HEADERS_ARM64: &[&str] = &[
        "clib/arch/AArch64/AArch64AddressingModes.h",
        "clib/arch/AArch64/AArch64BaseInfo.h",
        "clib/arch/AArch64/AArch64Disassembler.h",
        "clib/arch/AArch64/AArch64GenAsmWriter.inc",
        "clib/arch/AArch64/AArch64GenDisassemblerTables.inc",
        "clib/arch/AArch64/AArch64GenInstrInfo.inc",
        "clib/arch/AArch64/AArch64GenRegisterInfo.inc",
        "clib/arch/AArch64/AArch64GenSubtargetInfo.inc",
        "clib/arch/AArch64/AArch64InstPrinter.h",
        "clib/arch/AArch64/AArch64Mapping.h",
        "clib/arch/AArch64/AArch64MappingInsn.inc",
    ];

    build.define("CAPSTONE_HAS_ARM64", None);
    build.includes(uniq_dirs(HEADERS_ARM64));
    build.files(SOURCES_ARM64);

    track(SOURCES_ARM64);
    track(HEADERS_ARM64);
}

fn add_mips_support(build: &mut cc::Build) {
    const SOURCES_MIPS: &[&str] = &[
        "clib/arch/Mips/MipsDisassembler.c",
        "clib/arch/Mips/MipsInstPrinter.c",
        "clib/arch/Mips/MipsMapping.c",
        "clib/arch/Mips/MipsModule.c",
    ];

    const HEADERS_MIPS: &[&str] = &[
        "clib/arch/Mips/MipsDisassembler.h",
        "clib/arch/Mips/MipsGenAsmWriter.inc",
        "clib/arch/Mips/MipsGenDisassemblerTables.inc",
        "clib/arch/Mips/MipsGenInstrInfo.inc",
        "clib/arch/Mips/MipsGenRegisterInfo.inc",
        "clib/arch/Mips/MipsGenSubtargetInfo.inc",
        "clib/arch/Mips/MipsInstPrinter.h",
        "clib/arch/Mips/MipsMapping.h",
        "clib/arch/Mips/MipsMappingInsn.inc",
    ];

    build.define("CAPSTONE_HAS_MIPS", None);
    build.includes(uniq_dirs(HEADERS_MIPS));
    build.files(SOURCES_MIPS);

    track(SOURCES_MIPS);
    track(HEADERS_MIPS);
}

fn add_ppc_support(build: &mut cc::Build) {
    const SOURCES_PPC: &[&str] = &[
        "clib/arch/PowerPC/PPCDisassembler.c",
        "clib/arch/PowerPC/PPCInstPrinter.c",
        "clib/arch/PowerPC/PPCMapping.c",
        "clib/arch/PowerPC/PPCModule.c",
    ];

    const HEADERS_PPC: &[&str] = &[
        "clib/arch/PowerPC/PPCDisassembler.h",
        "clib/arch/PowerPC/PPCGenAsmWriter.inc",
        "clib/arch/PowerPC/PPCGenDisassemblerTables.inc",
        "clib/arch/PowerPC/PPCGenInstrInfo.inc",
        "clib/arch/PowerPC/PPCGenRegisterInfo.inc",
        "clib/arch/PowerPC/PPCGenSubtargetInfo.inc",
        "clib/arch/PowerPC/PPCInstPrinter.h",
        "clib/arch/PowerPC/PPCMapping.h",
        "clib/arch/PowerPC/PPCMappingInsn.inc",
        "clib/arch/PowerPC/PPCPredicates.h",
    ];

    build.define("CAPSTONE_HAS_POWERPC", None);
    build.includes(uniq_dirs(HEADERS_PPC));
    build.files(SOURCES_PPC);

    track(SOURCES_PPC);
    track(HEADERS_PPC);
}

fn add_x86_support(build: &mut cc::Build) {
    const SOURCES_X86: &[&str] = &[
        "clib/arch/X86/X86Disassembler.c",
        "clib/arch/X86/X86DisassemblerDecoder.c",
        "clib/arch/X86/X86IntelInstPrinter.c",
        "clib/arch/X86/X86Mapping.c",
        "clib/arch/X86/X86Module.c",
    ];

    const HEADERS_X86: &[&str] = &[
        "clib/arch/X86/X86BaseInfo.h",
        "clib/arch/X86/X86DisassemblerDecoderCommon.h",
        "clib/arch/X86/X86DisassemblerDecoder.h",
        "clib/arch/X86/X86Disassembler.h",
        "clib/arch/X86/X86GenAsmWriter1.inc",
        "clib/arch/X86/X86GenAsmWriter1_reduce.inc",
        "clib/arch/X86/X86GenAsmWriter.inc",
        "clib/arch/X86/X86GenAsmWriter_reduce.inc",
        "clib/arch/X86/X86GenDisassemblerTables.inc",
        "clib/arch/X86/X86GenDisassemblerTables_reduce.inc",
        "clib/arch/X86/X86GenInstrInfo.inc",
        "clib/arch/X86/X86GenInstrInfo_reduce.inc",
        "clib/arch/X86/X86GenRegisterInfo.inc",
        "clib/arch/X86/X86InstPrinter.h",
        "clib/arch/X86/X86Mapping.h",
        "clib/arch/X86/X86MappingInsn.inc",
        "clib/arch/X86/X86MappingInsnOp.inc",
        "clib/arch/X86/X86MappingInsnOp_reduce.inc",
        "clib/arch/X86/X86MappingInsn_reduce.inc",
    ];

    build.define("CAPSTONE_HAS_X86", None);
    build.includes(uniq_dirs(HEADERS_X86));
    build.files(SOURCES_X86);

    if !cfg!(feature = "diet") {
        build.file("clib/arch/X86/X86ATTInstPrinter.c");
        track(&["clib/arch/X86/X86ATTInstPrinter.c"]);
    }

    track(SOURCES_X86);
    track(HEADERS_X86);
}

fn add_sparc_support(build: &mut cc::Build) {
    const SOURCES_SPARC: &[&str] = &[
        "clib/arch/Sparc/SparcDisassembler.c",
        "clib/arch/Sparc/SparcInstPrinter.c",
        "clib/arch/Sparc/SparcMapping.c",
        "clib/arch/Sparc/SparcModule.c",
    ];

    const HEADERS_SPARC: &[&str] = &[
        "clib/arch/Sparc/Sparc.h",
        "clib/arch/Sparc/SparcDisassembler.h",
        "clib/arch/Sparc/SparcGenAsmWriter.inc",
        "clib/arch/Sparc/SparcGenDisassemblerTables.inc",
        "clib/arch/Sparc/SparcGenInstrInfo.inc",
        "clib/arch/Sparc/SparcGenRegisterInfo.inc",
        "clib/arch/Sparc/SparcGenSubtargetInfo.inc",
        "clib/arch/Sparc/SparcInstPrinter.h",
        "clib/arch/Sparc/SparcMapping.h",
        "clib/arch/Sparc/SparcMappingInsn.inc",
    ];

    build.define("CAPSTONE_HAS_SPARC", None);
    build.includes(uniq_dirs(HEADERS_SPARC));
    build.files(SOURCES_SPARC);

    track(SOURCES_SPARC);
    track(HEADERS_SPARC);
}

fn add_sysz_support(build: &mut cc::Build) {
    const SOURCES_SYSZ: &[&str] = &[
        "clib/arch/SystemZ/SystemZDisassembler.c",
        "clib/arch/SystemZ/SystemZInstPrinter.c",
        "clib/arch/SystemZ/SystemZMapping.c",
        "clib/arch/SystemZ/SystemZModule.c",
        "clib/arch/SystemZ/SystemZMCTargetDesc.c",
    ];

    const HEADERS_SYSZ: &[&str] = &[
        "clib/arch/SystemZ/SystemZDisassembler.h",
        "clib/arch/SystemZ/SystemZGenAsmWriter.inc",
        "clib/arch/SystemZ/SystemZGenDisassemblerTables.inc",
        "clib/arch/SystemZ/SystemZGenInsnNameMaps.inc",
        "clib/arch/SystemZ/SystemZGenInstrInfo.inc",
        "clib/arch/SystemZ/SystemZGenRegisterInfo.inc",
        "clib/arch/SystemZ/SystemZGenSubtargetInfo.inc",
        "clib/arch/SystemZ/SystemZInstPrinter.h",
        "clib/arch/SystemZ/SystemZMapping.h",
        "clib/arch/SystemZ/SystemZMappingInsn.inc",
        "clib/arch/SystemZ/SystemZMCTargetDesc.h",
    ];

    build.define("CAPSTONE_HAS_SYSZ", None);
    build.includes(uniq_dirs(HEADERS_SYSZ));
    build.files(SOURCES_SYSZ);

    track(SOURCES_SYSZ);
    track(HEADERS_SYSZ);
}

fn add_xcore_support(build: &mut cc::Build) {
    const SOURCES_XCORE: &[&str] = &[
        "clib/arch/XCore/XCoreDisassembler.c",
        "clib/arch/XCore/XCoreInstPrinter.c",
        "clib/arch/XCore/XCoreMapping.c",
        "clib/arch/XCore/XCoreModule.c",
    ];

    const HEADERS_XCORE: &[&str] = &[
        "clib/arch/XCore/XCoreDisassembler.h",
        "clib/arch/XCore/XCoreGenAsmWriter.inc",
        "clib/arch/XCore/XCoreGenDisassemblerTables.inc",
        "clib/arch/XCore/XCoreGenInstrInfo.inc",
        "clib/arch/XCore/XCoreGenRegisterInfo.inc",
        "clib/arch/XCore/XCoreInstPrinter.h",
        "clib/arch/XCore/XCoreMapping.h",
        "clib/arch/XCore/XCoreMappingInsn.inc",
    ];

    build.define("CAPSTONE_HAS_XCORE", None);
    build.includes(uniq_dirs(HEADERS_XCORE));
    build.files(SOURCES_XCORE);

    track(SOURCES_XCORE);
    track(HEADERS_XCORE);
}

fn add_m68k_support(build: &mut cc::Build) {
    const SOURCES_M68K: &[&str] = &[
        "clib/arch/M68K/M68KDisassembler.c",
        "clib/arch/M68K/M68KInstPrinter.c",
        "clib/arch/M68K/M68KModule.c",
    ];

    const HEADERS_M68K: &[&str] = &["clib/arch/M68K/M68KDisassembler.h"];

    build.define("CAPSTONE_HAS_M68K", None);
    build.includes(uniq_dirs(HEADERS_M68K));
    build.files(SOURCES_M68K);

    track(SOURCES_M68K);
    track(HEADERS_M68K);
}

fn add_tms320c64x_support(build: &mut cc::Build) {
    const SOURCES_TMS320C64X: &[&str] = &[
        "clib/arch/TMS320C64x/TMS320C64xDisassembler.c",
        "clib/arch/TMS320C64x/TMS320C64xInstPrinter.c",
        "clib/arch/TMS320C64x/TMS320C64xMapping.c",
        "clib/arch/TMS320C64x/TMS320C64xModule.c",
    ];

    const HEADERS_TMS320C64X: &[&str] = &[
        "clib/arch/TMS320C64x/TMS320C64xDisassembler.h",
        "clib/arch/TMS320C64x/TMS320C64xGenAsmWriter.inc",
        "clib/arch/TMS320C64x/TMS320C64xGenDisassemblerTables.inc",
        "clib/arch/TMS320C64x/TMS320C64xGenInstrInfo.inc",
        "clib/arch/TMS320C64x/TMS320C64xGenRegisterInfo.inc",
        "clib/arch/TMS320C64x/TMS320C64xInstPrinter.h",
        "clib/arch/TMS320C64x/TMS320C64xMapping.h",
    ];

    build.define("CAPSTONE_HAS_TMS320C64X", None);
    build.includes(uniq_dirs(HEADERS_TMS320C64X));
    build.files(SOURCES_TMS320C64X);

    track(SOURCES_TMS320C64X);
    track(HEADERS_TMS320C64X);
}

fn add_m680x_support(build: &mut cc::Build) {
    const SOURCES_M680X: &[&str] = &[
        "clib/arch/M680X/M680XDisassembler.c",
        "clib/arch/M680X/M680XInstPrinter.c",
        "clib/arch/M680X/M680XModule.c",
    ];

    const HEADERS_M680X: &[&str] = &[
        "clib/arch/M680X/M680XInstPrinter.h",
        "clib/arch/M680X/M680XDisassembler.h",
        "clib/arch/M680X/M680XDisassemblerInternals.h",
    ];

    build.define("CAPSTONE_HAS_M680X", None);
    build.includes(uniq_dirs(HEADERS_M680X));
    build.files(SOURCES_M680X);

    track(SOURCES_M680X);
    track(HEADERS_M680X);
}

fn add_evm_support(build: &mut cc::Build) {
    const SOURCES_EVM: &[&str] = &[
        "clib/arch/EVM/EVMDisassembler.c",
        "clib/arch/EVM/EVMInstPrinter.c",
        "clib/arch/EVM/EVMMapping.c",
        "clib/arch/EVM/EVMModule.c",
    ];

    const HEADERS_EVM: &[&str] = &[
        "clib/arch/EVM/EVMDisassembler.h",
        "clib/arch/EVM/EVMInstPrinter.h",
        "clib/arch/EVM/EVMMapping.h",
        "clib/arch/EVM/EVMMappingInsn.inc",
    ];

    build.define("CAPSTONE_HAS_EVM", None);
    build.includes(uniq_dirs(HEADERS_EVM));
    build.files(SOURCES_EVM);

    track(SOURCES_EVM);
    track(HEADERS_EVM);
}

fn add_mos65xx_support(build: &mut cc::Build) {
    const SOURCES_MOS65XX: &[&str] = &[
        "clib/arch/MOS65XX/MOS65XXModule.c",
        "clib/arch/MOS65XX/MOS65XXDisassembler.c",
    ];

    const HEADERS_MOS65XX: &[&str] = &["clib/arch/MOS65XX/MOS65XXDisassembler.h"];

    build.define("CAPSTONE_HAS_MOS65XX", None);
    build.includes(uniq_dirs(HEADERS_MOS65XX));
    build.files(SOURCES_MOS65XX);

    track(SOURCES_MOS65XX);
    track(HEADERS_MOS65XX);
}

fn uniq_dirs<'a>(dirs: &'a [&str]) -> Vec<&'a Path> {
    let mut uniq: Vec<&Path> = dirs.iter().filter_map(|f| Path::new(f).parent()).collect();
    uniq.sort();
    uniq.dedup();
    uniq
}

fn track(paths: &[&str]) {
    for p in paths {
        println!("cargo:rerun-if-changed={}", p);
    }
}
