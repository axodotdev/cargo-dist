# Quality Assurance Reports

This directory is produced by CI. For local reproduction:

```bash
rustup override set stable
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo deny check
cargo llvm-cov --workspace --all-features --html
```

Artifacts expected:
- `coverage/` (HTML report from llvm-cov)
- `lcov.info` (uploaded by CI)
- `clippy.json` or logs
- `cargo-deny` JSON report
- `lint.txt` (fmt check logs)
