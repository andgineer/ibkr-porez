# ibkr-porez

Serbian tax reporting for Interactive Brokers -- Rust rewrite of
[ibkr-porez (Python)](https://github.com/andgineer/ibkr-porez).

Functionally and database compatible with the Python version -- you can switch
between them without data loss.

**[Documentation](https://andgineer.github.io/ibkr-porez-rs/)**

## Migration Status

- [x] Models and storage (Python-compatible JSON)
- [x] IBKR API clients (Flex Query XML, CSV import)
- [x] NBS exchange rate client (with holiday calendar)
- [x] Tax calculations (FIFO, 10-year exemption)
- [x] PPDG-3R report generation (capital gains XML)
- [x] PP-OPO report generation (capital income XML)
- [x] CLI commands (clap derive mode)
- [x] GUI (egui/eframe, glow backend)
- [x] Packaging and installers (macOS PKG, Windows MSI)

## Installation

Download a prebuilt binary from the
[releases page](https://github.com/andgineer/ibkr-porez-rs/releases),
or install from source:

```sh
cargo install ibkr-porez
```

## Quick Start

```sh
# Configure personal data and IBKR access
ibkr-porez config

# Fetch latest data from IBKR and sync NBS exchange rates
ibkr-porez fetch

# Import historical CSV data (for transactions older than 1 year)
ibkr-porez import /path/to/activity_statement.csv

# Fetch data + create all due declarations
ibkr-porez sync

# Generate a specific tax report
ibkr-porez report

# List declarations
ibkr-porez list

# Submit / pay / export a declaration
ibkr-porez submit <id>
ibkr-porez pay <id>
ibkr-porez export <id>
```

See the full [usage guide](https://andgineer.github.io/ibkr-porez-rs/en/usage.html)
for all commands and options.

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, installed via `rustup`)
- `rustfmt` and `clippy` are installed automatically from `rust-toolchain.toml`

### Build

```sh
cargo build                          # CLI only (debug)
cargo build --features gui           # CLI + GUI (debug)
cargo build --release --features gui # optimized, stripped
```

Two binaries are produced (when built with `--features gui`):
- `target/release/ibkr-porez` -- CLI (also launches GUI when run without arguments)
- `target/release/gui` -- GUI

On Linux, building the GUI requires X11/Wayland and GTK development libraries:

```sh
sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libgtk-3-dev
```

### Tests

```sh
cargo test                 # CLI and library tests
cargo test --features gui  # all tests including GUI unit tests
```

### Linting and Formatting

```sh
cargo fmt --check
cargo clippy --features gui -- -D warnings
```

### Versioning and Release

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

### Documentation

Docs are built with [mdBook](https://rust-lang.github.io/mdBook/) (5 languages)
and deployed automatically to
[GitHub Pages](https://andgineer.github.io/ibkr-porez-rs/) on push to `main`.

```sh
# Build locally
bash scripts/build-docs.sh

# Serve locally (English only)
mdbook serve docs/en
```
