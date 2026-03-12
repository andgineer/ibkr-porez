# ibkr-porez

Serbian tax reporting for Interactive Brokers -- Rust rewrite of
[ibkr-porez (Python)](https://github.com/andgineer/ibkr-porez).

## Migration Status

- [ ] Models and storage
- [ ] IBKR API clients (Flex Query, CSV)
- [ ] NBS exchange rate client
- [ ] Tax calculations (FIFO)
- [ ] PPDG-3R report (capital gains)
- [ ] PP-OPO report (capital income)
- [ ] CLI commands
- [ ] GUI
- [ ] Packaging and installers

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, installed via `rustup`)
- `rustfmt` and `clippy` are installed automatically from `rust-toolchain.toml`

## Build

```sh
# Build everything (debug)
cargo build

# Build release binaries (optimized, stripped)
cargo build --release
```

Two binaries are produced:
- `target/release/ibkr-porez` -- CLI
- `target/release/gui` -- GUI

## Run

```sh
# Run CLI (default binary)
cargo run -- --help

# Run GUI
cargo run --bin gui
```

## Tests

```sh
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

## Linting and Formatting

```sh
# Check formatting (same as CI)
cargo fmt --check

# Auto-format
cargo fmt

# Run clippy linter (same as CI)
cargo clippy -- -D warnings
```

## Versioning and Release

The single source of truth for the version is `version` in `Cargo.toml`.

```sh
make version    # show current version
```

To create a release, pick the bump level:

```sh
make ver-bug      # 0.0.1 -> 0.0.2  (bug fix)
make ver-feature  # 0.0.2 -> 0.1.0  (new feature)
make ver-release  # 0.1.0 -> 1.0.0  (release)
```

This bumps the version in `Cargo.toml`, commits, and creates a git tag.
Then push to trigger the
[release workflow](.github/workflows/release.yml):

```sh
git push origin main v$(make version)
```

The workflow builds release binaries for Linux, macOS (x86_64 + aarch64),
and Windows, then publishes them as a GitHub Release.
