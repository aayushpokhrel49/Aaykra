> [!IMPORTANT]
> Remove this line to confirm you've reviewed this PR before submitting.

<p align="center">
  <img
    src="assets/images/aaykra_icon.png"
    alt="AAYKRA logo"
    width="160"
    height="160"
  />
</p>

<h1 align="center">AAYKRA</h1>

<p align="center">
  A fast, native code editor built in Rust with a GPU-accelerated renderer.
</p>

<p align="center">
  <a href="https://github.com/aayushpokhrel49/Aaykra/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/aayushpokhrel49/Aaykra?label=release&logo=github"></a>
  <a href="./LICENSE-GPL"><img alt="GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg"></a>
</p>

AAYKRA pairs the speed of a native application with the familiarity of a modern
IDE, and stays out of your way. Previously known as **Aayushi**.

## Features

- **Blazing fast** — GPU-accelerated rendering, incremental parsing, and a
  responsive interface built for large codebases.
- **Native feel** — real windows, native text rendering, and a polished UI on
  macOS, Linux, and Windows.
- **Multi-language support** — built-in grammars, tree-sitter syntax trees, and
  smart editor features.
- **Integrated tools** — terminal, task runner, debugging with DAP, language
  servers via LSP, and a built-in file explorer.
- **Run current file** — build and run the file in the active tab with `Ctrl-R`
  or `F5`, with output in the terminal panel.
- **Remote development** — pair with a remote host or WSL through the bundled
  remote-server binaries.
- **Extensions** — extend the editor with language and theme extensions.

## Install

Download the latest release for your platform from the
[releases page](https://github.com/aayushpokhrel49/Aaykra/releases) and follow
[INSTALL.md](./INSTALL.md). On macOS and Linux you can install the latest
release in one step:

```sh
curl -fsSL https://raw.githubusercontent.com/aayushpokhrel49/Aaykra/main/script/install.sh | sh
```

Set `ZED_VERSION=<x.y.z>` to pin a specific release.

| Platform | Architecture  | Artifact                              |
| -------- | ------------- | ------------------------------------- |
| macOS    | Apple Silicon | `aaykra-aarch64.dmg`                  |
| macOS    | Intel         | `aaykra-x86_64.dmg`                   |
| Linux    | x86_64        | `aaykra-linux-x86_64.tar.gz`          |
| Linux    | aarch64       | `aaykra-linux-aarch64.tar.gz`         |
| Arch     | x86_64        | `aaykra-<version>.pkg.tar.zst`        |
| Arch     | aarch64       | `aaykra-<version>.pkg.tar.zst`        |
| Debian   | x86_64        | `aaykra_<version>-<commit>_amd64.deb` |
| Debian   | aarch64       | `aaykra_<version>-<commit>_arm64.deb` |
| Windows  | x86_64        | `aaykra-x86_64.exe`                   |

## Build from source

Requires Rust `1.97.1` (pinned in `rust-toolchain.toml`) plus the platform
toolchain.

```sh
./script/linux            # Linux build dependencies
./script/bundle-linux     # .tar.gz for any Linux distro
./script/bundle-arch      # Arch Linux .pkg.tar.zst
./script/bundle-deb       # Debian/Ubuntu .deb

brew install lld          # macOS
./script/bundle-mac aarch64-apple-darwin

./script/bundle-windows.ps1 -Architecture x86_64   # Windows
```

For everyday development:

```sh
./script/clippy             # lint (preferred over `cargo clippy`)
cargo test -p <crate>       # tests for a single crate
```

Releases are cut with `script/release <version>` after adding a bullet under
`## Unreleased` in `.github/release/CHANGELOG.md`; pushing the `v*` tag runs
the release workflow.

## Contributing

Bug reports, feature ideas, and pull requests are welcome. Before opening a PR,
add a bullet describing your user-facing change under `## Unreleased` in
[.github/release/CHANGELOG.md](./.github/release/CHANGELOG.md), fill in
[.github/pull_request_template.md](./.github/pull_request_template.md), and run
`./script/clippy` along with the relevant tests.

## License

Distributed under the GPL-3.0-or-later and Apache-2.0 licenses. See
[LICENSE-GPL](./LICENSE-GPL) and [LICENSE-APACHE](./LICENSE-APACHE) for details.

## Credits

AAYKRA is a fork of [Zed](https://zed.dev), and would not exist without the
incredible editor built by [Zed Industries](https://github.com/zed-industries).
Most of this codebase originates from
[`zed-industries/zed`](https://github.com/zed-industries/zed) and is used under
its license terms, combined with other high-quality open source work from the
Rust ecosystem and the broader editor community. Thank you, Zed.
