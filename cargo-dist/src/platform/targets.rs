//! Information about various supported platforms

// Various target triples

use cargo_dist_schema::TargetTripleRef;

macro_rules! define_target_triples {
    ($($(#[$meta:meta])* const $name:ident = $triple:expr;)*) => {
        $(
            $(#[$meta])*
            pub const $name: &TargetTripleRef = TargetTripleRef::from_str($triple);
        )*
    };
}

define_target_triples!(
    /// 32-bit Windows MSVC (Windows 7+)
    const TARGET_X86_WINDOWS = "i686-pc-windows-msvc";
    /// 64-bit Windows MSVC (Windows 7+)
    const TARGET_X64_WINDOWS = "x86_64-pc-windows-msvc";
    /// ARM64 Windows MSVC
    const TARGET_ARM64_WINDOWS = "aarch64-pc-windows-msvc";
    /// 32-bit MinGW (Windows 7+)
    const TARGET_X86_MINGW = "i686-pc-windows-gnu";
    /// 64-bit MinGW (Windows 7+)
    const TARGET_X64_MINGW = "x86_64-pc-windows-gnu";
    /// ARM64 MinGW (Windows 7+)
    const TARGET_ARM64_MINGW = "aarch64-pc-windows-gnu";
);

/// List of all recognized Windows targets
pub const KNOWN_WINDOWS_TARGETS: &[&TargetTripleRef] = &[
    TARGET_X86_WINDOWS,
    TARGET_X64_WINDOWS,
    TARGET_ARM64_WINDOWS,
    TARGET_X86_MINGW,
    TARGET_X64_MINGW,
    TARGET_ARM64_MINGW,
];

define_target_triples!(
    /// 32-bit Intel macOS (10.12+, Sierra+)
    const TARGET_X86_MAC = "i686-apple-darwin";
    /// 64-bit Intel macOS (10.12+, Sierra+)
    const TARGET_X64_MAC = "x86_64-apple-darwin";
    /// ARM64 macOS (11.0+, Big Sur+) -- AKA "Apple Silicon"
    const TARGET_ARM64_MAC = "aarch64-apple-darwin";
);

/// List of all recognized Mac targets
pub const KNOWN_MAC_TARGETS: &[&TargetTripleRef] =
    &[TARGET_X86_MAC, TARGET_X64_MAC, TARGET_ARM64_MAC];

define_target_triples!(
    /// 32-bit Linux (kernel 3.2+, glibc 2.17+)
    const TARGET_X86_LINUX_GNU = "i686-unknown-linux-gnu";
    /// 64-bit Linux (kernel 3.2+, glibc 2.17+)
    const TARGET_X64_LINUX_GNU = "x86_64-unknown-linux-gnu";
    /// ARM64 Linux (kernel 4.1, glibc 2.17+)
    const TARGET_ARM64_LINUX_GNU = "aarch64-unknown-linux-gnu";
    /// ARMv7-A Linux, hardfloat (kernel 3.2, glibc 2.17) -- AKA ARMv7-A Linux
    const TARGET_ARMV7_LINUX_GNU = "armv7-unknown-linux-gnueabihf";
    /// ARMv6 Linux (kernel 3.2, glibc 2.17)
    const TARGET_ARMV6_LINUX_GNU = "arm-unknown-linux-gnueabi";
    /// ARMv6 Linux, hardfloat (kernel 3.2, glibc 2.17)
    const TARGET_ARMV6_LINUX_GNU_HARDFLOAT = "arm-unknown-linux-gnueabihf";
    /// PowerPC Linux (kernel 3.2, glibc 2.17)
    const TARGET_PPC_LINUX_GNU = "powerpc-unknown-linux-gnu";
    /// PPC64 Linux (kernel 3.2, glibc 2.17)
    const TARGET_PPC64_LINUX_GNU = "powerpc64-unknown-linux-gnu";
    /// PPC64LE Linux (kernel 3.10, glibc 2.17)
    const TARGET_PPC64LE_LINUX_GNU = "powerpc64le-unknown-linux-gnu";
    /// S390x Linux (kernel 3.2, glibc 2.17)
    const TARGET_S390X_LINUX_GNU = "s390x-unknown-linux-gnu";
    /// RISC-V Linux (kernel 4.20, glibc 2.29)
    const TARGET_RISCV_LINUX_GNU = "riscv64gc-unknown-linux-gnu";
    /// LoongArch64 Linux, LP64D ABI (kernel 5.19, glibc 2.36)
    const TARGET_LOONGARCH64_LINUX_GNU = "loongarch64-unknown-linux-gnu";
    /// SPARC Linux (kernel 4.4, glibc 2.23)
    const TARGET_SPARC64_LINUX_GNU = "sparc64-unknown-linux-gnu";
);

/// List of all recognized Linux glibc targets
pub const KNOWN_LINUX_GNU_TARGETS: &[&TargetTripleRef] = &[
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

define_target_triples!(
    /// 32-bit Linux with MUSL
    const TARGET_X86_LINUX_MUSL = "i686-unknown-linux-musl";
    /// 64-bit Linux with MUSL
    const TARGET_X64_LINUX_MUSL = "x86_64-unknown-linux-musl";
    /// ARM64 Linux with MUSL
    const TARGET_ARM64_LINUX_MUSL = "aarch64-unknown-linux-musl";
    /// ARMv7-A Linux with MUSL, hardfloat
    const TARGET_ARMV7_LINUX_MUSL = "armv7-unknown-linux-musleabihf";
    /// ARMv6 Linux with MUSL
    const TARGET_ARMV6_LINUX_MUSL = "arm-unknown-linux-musleabi";
    /// ARMv6 Linux with MUSL, hardfloat
    const TARGET_ARMV6_LINUX_MUSL_HARDFLOAT = "arm-unknown-linux-musleabihf";
    /// PowerPC Linux with MUSL
    const TARGET_PPC_LINUX_MUSL = "powerpc-unknown-linux-musl";
    /// PPC64 Linux with MUSL
    const TARGET_PPC64_LINUX_MUSL = "powerpc64-unknown-linux-musl";
    /// PPC64LE Linux with MUSL
    const TARGET_PPC64LE_LINUX_MUSL = "powerpc64le-unknown-linux-musl";
    /// S390x Linux with MUSL
    const TARGET_S390X_LINUX_MUSL = "s390x-unknown-linux-musl";
    /// RISC-V Linux with MUSL
    const TARGET_RISCV_LINUX_MUSL = "riscv64gc-unknown-linux-musl";
    /// LoongArch64 Linux with MUSL, LP64D ABI
    const TARGET_LOONGARCH64_LINUX_MUSL = "loongarch64-unknown-linux-musl";
    /// SPARC Linux with MUSL
    const TARGET_SPARC64_LINUX_MUSL = "sparc64-unknown-linux-musl";
);

/// List of all recognized Linux MUSL targets
pub const KNOWN_LINUX_MUSL_TARGETS: &[&TargetTripleRef] = &[
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
pub const KNOWN_LINUX_TARGETS: &[&[&TargetTripleRef]] =
    &[KNOWN_LINUX_GNU_TARGETS, KNOWN_LINUX_MUSL_TARGETS];

define_target_triples!(
    /// 64-bit FreeBSD
    const TARGET_X64_FREEBSD = "x86_64-unknown-freebsd";
    /// illumos
    const TARGET_X64_ILLUMOS = "x86_64-unknown-illumos";
    /// NetBSD/amd64
    const TARGET_X64_NETBSD = "x86_64-unknown-netbsd";
    /// ARM64 iOS
    const TARGET_ARM64_IOS = "aarch64-apple-ios";
    /// ARM64 Fuchsia
    const TARGET_ARM64_FUCHSIA = "aarch64-unknown-fuchsia";
    /// ARM64 Android
    const TARGET_ARM64_ANDROID = "aarch64-linux-android";
    /// 64-bit x86 Android
    const TARGET_X64_ANDROID = "x86_64-linux-android";
    /// WebAssembly with WASI
    const TARGET_WASM32_WASI = "wasm32-wasi";
    /// WebAssembly
    const TARGET_WASM32 = "wasm32-unknown-unknown";
    /// SPARC Solaris 10/11, illumos
    const TARGET_SPARC_SOLARIS = "sparcv9-sun-solaris";
    /// 64-bit Solaris 10/11, illumos
    const TARGET_X64_SOLARIS = "x86_64-pc-solaris";
);

/// List of all recognized Other targets
pub const KNOWN_OTHER_TARGETS: &[&TargetTripleRef] = &[
    TARGET_X64_FREEBSD,
    TARGET_X64_ILLUMOS,
    TARGET_X64_NETBSD,
    TARGET_ARM64_IOS,
    TARGET_ARM64_FUCHSIA,
    TARGET_ARM64_ANDROID,
    TARGET_X64_ANDROID,
    TARGET_WASM32_WASI,
    TARGET_WASM32,
    TARGET_SPARC_SOLARIS,
    TARGET_X64_SOLARIS,
];

/// List of all recognized targets
pub const KNOWN_TARGET_TRIPLES: &[&[&TargetTripleRef]] = &[
    KNOWN_WINDOWS_TARGETS,
    KNOWN_MAC_TARGETS,
    KNOWN_LINUX_GNU_TARGETS,
    KNOWN_LINUX_MUSL_TARGETS,
    KNOWN_OTHER_TARGETS,
];

/// The current host target (the target of the machine this code is running on).
/// This is determined through `std::env::consts::OS` rather than running `cargo`
pub const TARGET_HOST: &TargetTripleRef = TargetTripleRef::from_str(std::env::consts::OS);
