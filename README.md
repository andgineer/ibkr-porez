[![CI](https://github.com/andgineer/ibkr-porez/actions/workflows/ci.yml/badge.svg)](https://github.com/andgineer/ibkr-porez/actions/workflows/ci.yml)
[![Coverage](https://raw.githubusercontent.com/andgineer/ibkr-porez/coverage-data/badge.svg)](https://htmlpreview.github.io/?https://github.com/andgineer/ibkr-porez/blob/coverage-data/htmlcov/index.html)

# ibkr-porez

Automated PPDG-3R and PP-OPO tax reports generation for Interactive Brokers.
It automatically fetches your data and generates a ready-to-upload XML files with all prices converted to RSD.

# Quick Start

Graphical installers are available for Windows and macOS.

[Install ibkr-porez](https://andgineer.github.io/ibkr-porez/en/installation.html)

If you use the graphical interface, configure your data (the `Config` button),
then just use `Sync` to refresh data and create declarations.

If CLI is your native language (AI agents and brave humans), follow
[CLI documentation](https://andgineer.github.io/ibkr-porez/).

---

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
- `target/release/ibkr-porez-gui` -- GUI

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

### IDE Setup (VS Code / Cursor)

The `gui` feature is not enabled by default, so rust-analyzer won't index GUI code
out of the box. Add this to your workspace settings (`.vscode/settings.json`):

```json
{ "rust-analyzer.cargo.features": ["gui"] }
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
[GitHub Pages](https://andgineer.github.io/ibkr-porez/) on push to `main`.

```sh
# Build locally
bash scripts/build-docs.sh

# Serve locally (English only)
mdbook serve docs/en
```
