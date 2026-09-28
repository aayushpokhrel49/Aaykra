<p align="center">
  <img
    src="assets/images/aaykra_icon.png"
    alt="AAYKRA logo"
    width="160"
    height="160"
  />
</p>

# AAYKRA

AAYKRA is a fast, native code editor built in Rust with a GPU-accelerated
renderer. It pairs the speed of a native application with the familiarity of a
modern IDE, and stays out of your way.

Previously known as **Aayushi**.

## Features

- **Blazing fast** — GPU-accelerated rendering, incremental parsing, and a
  responsive interface built for large codebases.
- **Native feel** — real windows, native text rendering, and a polished UI on
  macOS, Linux, and Windows.
- **Multi-language support** — built-in grammars, tree-sitter syntax trees,
  and smart editor features.
- **Integrated tools** — terminal, task runner, debugging with DAP, language
  servers via LSP, and a built-in file explorer.
- **Extensions** — extend the editor with language and theme extensions.
- **Open standard** — light enough to run anywhere, powerful enough for daily
  development.

## Platforms

AAYKRA builds stable releases for:

| Platform  | Architecture | Artifact                            |
| --------- | ------------ | ----------------------------------- |
| macOS     | Apple Silicon | `aaykra-aarch64.dmg`              |
| macOS     | Intel        | `aaykra-x86_64.dmg`               |
| Linux     | x86_64       | `aaykra-linux-x86_64.tar.gz`      |
| Linux     | aarch64      | `aaykra-linux-aarch64.tar.gz`     |
| Arch      | x86_64       | `aaykra-<version>.pkg.tar.zst`    |
| Arch      | aarch64      | `aaykra-<version>.pkg.tar.zst`    |
| Windows   | x86_64       | `aaykra-x86_64.exe`               |

A `.deb` package (`aaykra_*.deb`) is also published for Debian/Ubuntu-based distributions.

## Install

Download the latest release from the
[releases page](https://github.com/aayushpokhrel49/Aaykra/releases) and
follow the instructions in [INSTALL.md](./INSTALL.md).

## Build from source

Prerequisites: Rust (via `rustup`) and the platform toolchains described below.

### Linux / Arch Linux

Install build dependencies:

```sh
./script/linux
```

Build a `.tar.gz` (for any Linux distro):

```sh
./script/bundle-linux
```

Build an Arch Linux `.pkg.tar.zst`:

```sh
./script/bundle-arch
```

Build a `.deb` (for Debian/Ubuntu-based distros):

```sh
./script/bundle-deb
```

### macOS

```sh
brew install lld
./script/bundle-mac aarch64-apple-darwin   # Apple Silicon
./script/bundle-mac x86_64-apple-darwin    # Intel
```

### Windows

```powershell
./script/bundle-windows.ps1 -Architecture x86_64
```

All release artifacts are also produced automatically by the GitHub Actions
workflow when a `v*` tag is pushed.

## Contributing

Bug reports, feature ideas, and pull requests are welcome. Open an issue or
submit a PR on the [repository](https://github.com/aayushpokhrel49/Aaykra).

## License

Distributed under the GPL-3.0-or-later and Apache-2.0 licenses. See
`LICENSE-GPL` and `LICENSE-APACHE` for details.

## Credits

AAYKRA is a fork of [Zed](https://zed.dev), and would not exist without
the incredible editor built by [Zed Industries](https://github.com/zed-industries).
Most of this codebase originates from
[`zed-industries/zed`](https://github.com/zed-industries/zed) and is used under
its license terms, combined with other high-quality open source work from the
Rust ecosystem and the broader editor community. Thank you, Zed.

- Zed: <https://github.com/zed-industries/zed>
- License: <https://github.com/zed-industries/zed/blob/main/LICENSE>
