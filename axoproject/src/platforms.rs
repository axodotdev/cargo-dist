//! Information about various supported platforms

// Various target triples
/// 32-bit Intel Windows
pub const TARGET_X86_WINDOWS: &str = "i686-pc-windows-msvc";
/// 64-bit Intel Windows
pub const TARGET_X64_WINDOWS: &str = "x86_64-pc-windows-msvc";
/// 64-bit ARM Windows
pub const TARGET_ARM64_WINDOWS: &str = "aarch64-pc-windows-msvc";

/// List of all recognized Windows targets
pub const KNOWN_WINDOWS_TARGETS: &[&str] =
    &[TARGET_X86_WINDOWS, TARGET_X64_WINDOWS, TARGET_ARM64_WINDOWS];

/// 32-bit Intel macOS
pub const TARGET_X86_MAC: &str = "i686-apple-darwin";
/// 64-bit Intel macOS
pub const TARGET_X64_MAC: &str = "x86_64-apple-darwin";
/// 64-bit Apple Silicon macOS
pub const TARGET_ARM64_MAC: &str = "aarch64-apple-darwin";

/// List of all recognized Mac targets
pub const KNOWN_MAC_TARGETS: &[&str] = &[TARGET_X86_MAC, TARGET_X64_MAC, TARGET_ARM64_MAC];

/// 32-bit Intel glibc Linux
pub const TARGET_X86_LINUX_GNU: &str = "i686-unknown-linux-gnu";
/// 64-bit Intel glibc Linux
pub const TARGET_X64_LINUX_GNU: &str = "x86_64-unknown-linux-gnu";
/// 64-bit ARM glibc Linux
pub const TARGET_ARM64_LINUX_GNU: &str = "aarch64-unknown-linux-gnu";
/// List of all recognized Linux glibc targets
pub const KNOWN_LINUX_GNU_TARGETS: &[&str] = &[
    TARGET_X86_LINUX_GNU,
    TARGET_X64_LINUX_GNU,
    TARGET_ARM64_LINUX_GNU,
];

/// 32-bit Intel musl Linux
pub const TARGET_X86_LINUX_MUSL: &str = "i686-unknown-linux-musl";
/// 64-bit Intel musl Linux
pub const TARGET_X64_LINUX_MUSL: &str = "x86_64-unknown-linux-musl";
/// 64-bit ARM musl Linux
pub const TARGET_ARM64_LINUX_MUSL: &str = "aarch64-unknown-linux-musl";
/// List of all recognized Linux musl targets
pub const KNOWN_LINUX_MUSL_TARGETS: &[&str] = &[
    TARGET_X86_LINUX_MUSL,
    TARGET_X64_LINUX_MUSL,
    TARGET_ARM64_LINUX_MUSL,
];

/// List of all recognized Linux targets
pub const KNOWN_LINUX_TARGETS: &[&[&str]] = &[KNOWN_LINUX_GNU_TARGETS, KNOWN_LINUX_MUSL_TARGETS];

/// List of all recognized targets
pub const KNOWN_TARGET_TRIPLES: &[&[&str]] = &[
    KNOWN_WINDOWS_TARGETS,
    KNOWN_MAC_TARGETS,
    KNOWN_LINUX_GNU_TARGETS,
    KNOWN_LINUX_MUSL_TARGETS,
];

/// Translates a Rust triple into a human-readable display name
pub fn triple_to_display_name(name: &str) -> Option<&str> {
    match name.trim() {
        TARGET_X86_LINUX_GNU => Some("Linux x86"),
        TARGET_X64_LINUX_GNU => Some("Linux x64"),
        TARGET_ARM64_LINUX_GNU => Some("Linux arm64"),

        TARGET_X86_LINUX_MUSL => Some("musl Linux x86"),
        TARGET_X64_LINUX_MUSL => Some("musl Linux x64"),
        TARGET_ARM64_LINUX_MUSL => Some("musl Linux arm64"),

        TARGET_X86_WINDOWS => Some("Windows x86"),
        TARGET_X64_WINDOWS => Some("Windows x64"),
        TARGET_ARM64_WINDOWS => Some("Windows arm64"),

        TARGET_X86_MAC => Some("macOS x86"),
        TARGET_X64_MAC => Some("macOS Intel"),
        TARGET_ARM64_MAC => Some("macOS Apple Silicon"),

        "all" => Some("All Platforms"),

        _ => None,
    }
}
