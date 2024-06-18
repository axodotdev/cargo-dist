//! Information about various supported platforms

// Various target triples
/// 32-bit Windows MSVC (Windows 7+)
pub const TARGET_X86_WINDOWS: &str = "i686-pc-windows-msvc";
/// 64-bit Windows MSVC (Windows 7+)
pub const TARGET_X64_WINDOWS: &str = "x86_64-pc-windows-msvc";
/// ARM64 Windows MSVC
pub const TARGET_ARM64_WINDOWS: &str = "aarch64-pc-windows-msvc";
/// 32-bit MinGW (Windows 7+)
pub const TARGET_X86_MINGW: &str = "i686-pc-windows-gnu";
/// 64-bit MinGW (Windows 7+)
pub const TARGET_X64_MINGW: &str = "x86_64-pc-windows-gnu";
/// ARM64 MinGW (Windows 7+)
pub const TARGET_ARM64_MINGW: &str = "aarch64-pc-windows-gnu";

/// List of all recognized Windows targets
pub const KNOWN_WINDOWS_TARGETS: &[&str] = &[
    TARGET_X86_WINDOWS,
    TARGET_X64_WINDOWS,
    TARGET_ARM64_WINDOWS,
    TARGET_X86_MINGW,
    TARGET_X64_MINGW,
    TARGET_ARM64_MINGW,
];

/// 32-bit Intel macOS (10.12+, Sierra+)
pub const TARGET_X86_MAC: &str = "i686-apple-darwin";
/// 64-bit Intel macOS (10.12+, Sierra+)
pub const TARGET_X64_MAC: &str = "x86_64-apple-darwin";
/// ARM64 macOS (11.0+, Big Sur+) -- AKA "Apple Silicon"
pub const TARGET_ARM64_MAC: &str = "aarch64-apple-darwin";

/// List of all recognized Mac targets
pub const KNOWN_MAC_TARGETS: &[&str] = &[TARGET_X86_MAC, TARGET_X64_MAC, TARGET_ARM64_MAC];

/// 32-bit Linux (kernel 3.2+, glibc 2.17+)
pub const TARGET_X86_LINUX_GNU: &str = "i686-unknown-linux-gnu";
/// 64-bit Linux (kernel 3.2+, glibc 2.17+)
pub const TARGET_X64_LINUX_GNU: &str = "x86_64-unknown-linux-gnu";
/// ARM64 Linux (kernel 4.1, glibc 2.17+)
pub const TARGET_ARM64_LINUX_GNU: &str = "aarch64-unknown-linux-gnu";
/// ARMv7-A Linux, hardfloat (kernel 3.2, glibc 2.17) -- AKA ARMv7-A Linux
pub const TARGET_ARMV7_LINUX_GNU: &str = "armv7-unknown-linux-gnueabihf";
/// ARMv6 Linux (kernel 3.2, glibc 2.17)
pub const TARGET_ARMV6_LINUX_GNU: &str = "arm-unknown-linux-gnueabi";
/// ARMv6 Linux, hardfloat (kernel 3.2, glibc 2.17)
pub const TARGET_ARMV6_LINUX_GNU_HARDFLOAT: &str = "arm-unknown-linux-gnueabihf";
/// PowerPC Linux (kernel 3.2, glibc 2.17)
pub const TARGET_PPC_LINUX_GNU: &str = "powerpc-unknown-linux-gnu";
/// PPC64 Linux (kernel 3.2, glibc 2.17)
pub const TARGET_PPC64_LINUX_GNU: &str = "powerpc64-unknown-linux-gnu";
/// PPC64LE Linux (kernel 3.10, glibc 2.17)
pub const TARGET_PPC64LE_LINUX_GNU: &str = "powerpc64le-unknown-linux-gnu";
/// S390x Linux (kernel 3.2, glibc 2.17)
pub const TARGET_S390X_LINUX_GNU: &str = "s390x-unknown-linux-gnu";
/// RISC-V Linux (kernel 4.20, glibc 2.29)
pub const TARGET_RISCV_LINUX_GNU: &str = "riscv64gc-unknown-linux-gnu";
/// LoongArch64 Linux, LP64D ABI (kernel 5.19, glibc 2.36)
pub const TARGET_LOONGARCH64_LINUX_GNU: &str = "loongarch64-unknown-linux-gnu";
/// SPARC Linux (kernel 4.4, glibc 2.23)
pub const TARGET_SPARC64_LINUX_GNU: &str = "sparc64-unknown-linux-gnu";

/// List of all recognized Linux glibc targets
pub const KNOWN_LINUX_GNU_TARGETS: &[&str] = &[
    TARGET_X86_LINUX_GNU,
    TARGET_X64_LINUX_GNU,
    TARGET_ARM64_LINUX_GNU,
    TARGET_ARMV7_LINUX_GNU,
    TARGET_ARMV6_LINUX_GNU,
    TARGET_ARMV6_LINUX_GNU_HARDFLOAT,
    TARGET_PPC64_LINUX_GNU,
    TARGET_PPC64LE_LINUX_GNU,
    TARGET_S390X_LINUX_GNU,
    TARGET_RISCV_LINUX_GNU,
    TARGET_LOONGARCH64_LINUX_GNU,
    TARGET_SPARC64_LINUX_GNU,
];

/// 32-bit Linux with MUSL
pub const TARGET_X86_LINUX_MUSL: &str = "i686-unknown-linux-musl";
/// 64-bit Linux with MUSL
pub const TARGET_X64_LINUX_MUSL: &str = "x86_64-unknown-linux-musl";
/// ARM64 Linux with MUSL
pub const TARGET_ARM64_LINUX_MUSL: &str = "aarch64-unknown-linux-musl";
/// ARMv7-A Linux with MUSL, hardfloat
pub const TARGET_ARMV7_LINUX_MUSL: &str = "armv7-unknown-linux-musleabihf";
/// ARMv6 Linux with MUSL
pub const TARGET_ARMV6_LINUX_MUSL: &str = "arm-unknown-linux-musleabi";
/// ARMv6 Linux with MUSL, hardfloat
pub const TARGET_ARMV6_LINUX_MUSL_HARDFLOAT: &str = "arm-unknown-linux-musleabihf";
/// PowerPC Linux with MUSL
pub const TARGET_PPC_LINUX_MUSL: &str = "powerpc-unknown-linux-musl";
/// PPC64 Linux with MUSL
pub const TARGET_PPC64_LINUX_MUSL: &str = "powerpc64-unknown-linux-musl";
/// PPC64LE Linux with MUSL
pub const TARGET_PPC64LE_LINUX_MUSL: &str = "powerpc64le-unknown-linux-musl";
/// S390x Linux with MUSL
pub const TARGET_S390X_LINUX_MUSL: &str = "s390x-unknown-linux-musl";
/// RISC-V Linux with MUSL
pub const TARGET_RISCV_LINUX_MUSL: &str = "riscv64gc-unknown-linux-musl";
/// LoongArch64 Linux with MUSL, LP64D ABI
pub const TARGET_LOONGARCH64_LINUX_MUSL: &str = "loongarch64-unknown-linux-musl";
/// SPARC Linux with MUSL
pub const TARGET_SPARC64_LINUX_MUSL: &str = "sparc64-unknown-linux-musl";

/// List of all recognized Linux MUSL targets
pub const KNOWN_LINUX_MUSL_TARGETS: &[&str] = &[
    TARGET_X86_LINUX_MUSL,
    TARGET_X64_LINUX_MUSL,
    TARGET_ARM64_LINUX_MUSL,
    TARGET_ARMV7_LINUX_MUSL,
    TARGET_ARMV6_LINUX_MUSL,
    TARGET_ARMV6_LINUX_MUSL_HARDFLOAT,
    TARGET_PPC64_LINUX_MUSL,
    TARGET_PPC64LE_LINUX_MUSL,
    TARGET_S390X_LINUX_MUSL,
    TARGET_RISCV_LINUX_MUSL,
    TARGET_LOONGARCH64_LINUX_MUSL,
    TARGET_SPARC64_LINUX_MUSL,
];

/// List of all recognized Linux targets
pub const KNOWN_LINUX_TARGETS: &[&[&str]] = &[KNOWN_LINUX_GNU_TARGETS, KNOWN_LINUX_MUSL_TARGETS];

/// 64-bit FreeBSD
pub const TARGET_X64_FREEBSD: &str = "x86_64-unknown-freebsd";
/// illumos
pub const TARGET_X64_ILLUMOS: &str = "x86_64-unknown-illumos";
/// NetBSD/amd64
pub const TARGET_X64_NETBSD: &str = "x86_64-unknown-netbsd";
/// ARM64 iOS
pub const TARGET_ARM64_IOS: &str = "aarch64-apple-ios";
/// Apple iOS Simulator on ARM64
pub const TARGET_ARM64_IOS_SIM: &str = "aarch64-apple-ios-sum";
/// 64-bit x86 iOS (simulator)
pub const TARGET_X64_IOS: &str = "x86_64-apple-ios";
/// ARM64 Fuchsia
pub const TARGET_ARM64_FUCHSIA: &str = "aarch64-unknown-fuchsia";
/// ARM64 Android
pub const TARGET_ARM64_ANDROID: &str = "aarch64-linux-android";
/// 64-bit x86 Android
pub const TARGET_X64_ANDROID: &str = "x86_64-linux-android";
/// asm.js via Emscripten
pub const TARGET_ASMJS_EMSCRIPTEN: &str = "asm.js via Emscripten";
/// WebAssembly with WASI
pub const TARGET_WASM32_WASI: &str = "wasm32-wasi";
/// WebAssembly
pub const TARGET_WASM32: &str = "wasm32-unknown-unknown";
/// SPARC Solaris 10/11, illumos
pub const TARGET_SPARC_SOLARIS: &str = "sparcv9-sun-solaris";
/// 64-bit Solaris 10/11, illumos
pub const TARGET_X64_SOLARIS: &str = "x86_64-pc-solaris";

/// List of all recognized Other targets
pub const KNOWN_OTHER_TARGETS: &[&str] = &[
    TARGET_X64_FREEBSD,
    TARGET_X64_ILLUMOS,
    TARGET_X64_NETBSD,
    TARGET_ARM64_IOS,
    TARGET_ARM64_IOS_SIM,
    TARGET_X64_IOS,
    TARGET_ARM64_FUCHSIA,
    TARGET_ARM64_ANDROID,
    TARGET_X64_ANDROID,
    TARGET_ASMJS_EMSCRIPTEN,
    TARGET_WASM32_WASI,
    TARGET_WASM32,
    TARGET_SPARC_SOLARIS,
    TARGET_X64_SOLARIS,
];

/// List of all recognized targets
pub const KNOWN_TARGET_TRIPLES: &[&[&str]] = &[
    KNOWN_WINDOWS_TARGETS,
    KNOWN_MAC_TARGETS,
    KNOWN_LINUX_GNU_TARGETS,
    KNOWN_LINUX_MUSL_TARGETS,
    KNOWN_OTHER_TARGETS,
];

/// Translates a Rust triple into a human-readable display name
pub fn triple_to_display_name(name: &str) -> Option<&str> {
    match name.trim() {
        TARGET_X86_LINUX_GNU => Some("x86 Linux"),
        TARGET_X64_LINUX_GNU => Some("x64 Linux"),
        TARGET_ARM64_LINUX_GNU => Some("ARM64 Linux"),
        TARGET_ARMV7_LINUX_GNU => Some("ARMv7 Linux"),
        TARGET_ARMV6_LINUX_GNU => Some("ARMv6 Linux"),
        TARGET_ARMV6_LINUX_GNU_HARDFLOAT => Some("ARMv6 Linux (Hardfloat)"),
        TARGET_PPC64_LINUX_GNU => Some("PPC64 Linux"),
        TARGET_PPC64LE_LINUX_GNU => Some("PPC64LE Linux"),
        TARGET_S390X_LINUX_GNU => Some("S390x Linux"),
        TARGET_RISCV_LINUX_GNU => Some("RISCV Linux"),
        TARGET_LOONGARCH64_LINUX_GNU => Some("LOONGARCH64 Linux"),
        TARGET_SPARC64_LINUX_GNU => Some("SPARC64 Linux"),

        TARGET_X86_LINUX_MUSL => Some("x86 MUSL Linux"),
        TARGET_X64_LINUX_MUSL => Some("x64 MUSL Linux"),
        TARGET_ARM64_LINUX_MUSL => Some("ARM64 MUSL Linux"),
        TARGET_ARMV7_LINUX_MUSL => Some("ARMv7 MUSL Linux"),
        TARGET_ARMV6_LINUX_MUSL => Some("ARMv6 MUSL Linux"),
        TARGET_ARMV6_LINUX_MUSL_HARDFLOAT => Some("ARMv6 MUSL Linux (Hardfloat)"),
        TARGET_PPC64_LINUX_MUSL => Some("PPC64 MUSL Linux"),
        TARGET_PPC64LE_LINUX_MUSL => Some("PPC64LE MUSL Linux"),
        TARGET_S390X_LINUX_MUSL => Some("S390x MUSL Linux"),
        TARGET_RISCV_LINUX_MUSL => Some("RISCV MUSL Linux"),
        TARGET_LOONGARCH64_LINUX_MUSL => Some("LOONGARCH64 MUSL Linux"),
        TARGET_SPARC64_LINUX_MUSL => Some("SPARC64 MUSL Linux"),

        TARGET_X86_WINDOWS => Some("x86 Windows"),
        TARGET_X64_WINDOWS => Some("x64 Windows"),
        TARGET_ARM64_WINDOWS => Some("ARM64 Windows"),
        TARGET_X86_MINGW => Some("x86 MinGW"),
        TARGET_X64_MINGW => Some("x64 MinGW"),
        TARGET_ARM64_MINGW => Some("ARM64 MinGW"),

        TARGET_X86_MAC => Some("x86 macOS"),
        TARGET_X64_MAC => Some("Intel macOS"),
        TARGET_ARM64_MAC => Some("Apple Silicon macOS"),

        TARGET_X64_FREEBSD => Some("x64 FreeBSD"),
        TARGET_X64_ILLUMOS => Some("x64 IllumOS"),
        TARGET_X64_NETBSD => Some("x64 NetBSD"),
        TARGET_ARM64_IOS => Some("iOS"),
        TARGET_ARM64_IOS_SIM => Some("ARM64 iOS SIM"),
        TARGET_X64_IOS => Some("x64 iOS"),
        TARGET_ARM64_FUCHSIA => Some("ARM64 Fuchsia"),
        TARGET_ARM64_ANDROID => Some("Android"),
        TARGET_X64_ANDROID => Some("x64 Android"),
        TARGET_ASMJS_EMSCRIPTEN => Some("asm.js Emscripten"),
        TARGET_WASM32_WASI => Some("WASI"),
        TARGET_WASM32 => Some("WASM"),
        TARGET_SPARC_SOLARIS => Some("SPARC Solaris"),
        TARGET_X64_SOLARIS => Some("x64 Solaris"),

        "all" => Some("All Platforms"),

        _ => None,
    }
}
