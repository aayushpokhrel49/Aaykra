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
  <a href="https://github.com/aayushpokhrel49/Aaykra/actions/workflows/release.yml"><img alt="release" src="https://img.shields.io/github/actions/workflow/status/aayushpokhrel49/Aaykra/release.yml?branch=main&label=release%20CI&logo=github"></a>
  <a href="https://github.com/aayushpokhrel49/Aaykra/actions/workflows/upstream-sync.yml"><img alt="upstream-sync" src="https://img.shields.io/github/actions/workflow/status/aayushpokhrel49/Aaykra/upstream-sync.yml?label=upstream%20sync&logo=github"></a>
  <a href="./LICENSE-GPL"><img alt="GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg"></a>
</p>

AAYKRA pairs the speed of a native application with the familiarity of a modern
IDE, and stays out of your way. Previously known as **Aayushi**.

## Features

- **Blazing fast** — GPU-accelerated rendering, incremental parsing, and a
  responsive interface built for large codebases.
- **Native feel** — real windows, native text rendering, and a polished UI on
  macOS, Linux, and Windows.
- **Multi-language support** — built-in grammars, tree-sitter syntax trees,
  and smart editor features.
- **Integrated tools** — terminal, task runner, debugging with DAP, language
  servers via LSP, and a built-in file explorer.
- **Remote development** — pair with a remote host or WSL through the bundled
  `aaykra-remote-server-*` binaries.
- **Extensions** — extend the editor with language and theme extensions.
- **Open standard** — light enough to run anywhere, powerful enough for daily
  development.

## Platforms

AAYKRA builds stable releases for:

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

Every platform build also ships a compressed
`aaykra-remote-server-<platform>-<arch>.gz` for remote development.

## Install

Download the latest release from the
[releases page](https://github.com/aayushpokhrel49/Aaykra/releases) and
follow the instructions in [INSTALL.md](./INSTALL.md). On macOS and Linux you can
also install the latest release in one step:

```sh
curl -fsSL https://raw.githubusercontent.com/aayushpokhrel49/Aaykra/main/script/install.sh | sh
```

Set `ZED_VERSION=<x.y.z>` to pin a specific release.

## Build from source

Prerequisites: Rust `1.97.1` (via `rustup`, pinned in `rust-toolchain.toml`) and
the platform toolchains described below.

```sh
./script/clippy             # lint (preferred over `cargo clippy`)
cargo test -p <crate>       # tests for a single crate
```

### Linux / Arch / Debian

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

## CI/CD

Every workflow lives in [`.github/workflows`](./.github/workflows). Builds run
against the pinned toolchain and write `stable` to
`crates/aaykra/RELEASE_CHANNEL` so bundles ship on the release channel.

### Workflows

| Workflow                                                 | Trigger                             | Runner(s)                                                                                | Produces                                                                                  |
| -------------------------------------------------------- | ----------------------------------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| [`release`](./.github/workflows/release.yml)             | `v*` tag push, manual dispatch      | `macos-15`, `macos-15-intel`, `ubuntu-22.04(-arm)`, `ubuntu-24.04(-arm)`, `windows-2022` | `.dmg`, `.tar.gz`, `.deb`, `.pkg.tar.zst`, `.exe`, remote server binaries, GitHub release |
| [`build-deb`](./.github/workflows/build-deb.yml)         | push to `main`, manual dispatch     | `ubuntu-24.04`                                                                           | `.deb` build artifact (verified with `dpkg-deb`)                                          |
| [`build-windows`](./.github/workflows/build-windows.yml) | push to `main`, manual dispatch     | `windows-2022`                                                                           | `.exe` build artifact                                                                     |
| [`upstream-sync`](./.github/workflows/upstream-sync.yml) | daily at 06:00 UTC, manual dispatch | `ubuntu-latest`                                                                          | `sync/<tag>` branch and a pull request                                                    |

### Release pipeline

`release.yml` fans out from a single `check_version` gate and only publishes if
every bundle succeeds:

1. **`check_version`** — resolves the version from the tag (or the manual
   `version` input), fails if it does not match `crates/aaykra/Cargo.toml`, and
   fails if `.github/release/CHANGELOG.md` has no section for that version.
2. **Bundle jobs** — `bundle_mac`, `bundle_linux`, `bundle_deb`,
   `bundle_arch_linux`, and `bundle_windows` each build one or two targets in
   parallel (`fail-fast: false`, so one architecture failing does not cancel
   the rest) and upload their artifacts.
3. **`release`** — downloads every artifact, drops the `.dSYM.zip` bundles so
   they stay out of the published assets, builds the release body from
   `.github/release/body.md` plus the matching changelog section and a
   compare link to the previous tag, then publishes the release with
   `softprops/action-gh-release`.

Each build job has a 300-minute timeout, and the macOS jobs publish `.dSYM.zip`
files as separate artifacts for crash symbolication.

### Upstream sync

Because AAYKRA tracks Zed, [`upstream-sync`](./.github/workflows/upstream-sync.yml)
runs once a day. It compares `UPSTREAM_VERSION` against the newest stable Zed
tag, runs `script/upstream-sync`, and opens a pull request from `sync/<tag>`
with the sync log pasted into the body:

- `Sync Zed <tag>` — upstream applied cleanly, build and test locally before
  merging.
- `Sync Zed <tag> (needs resolution)` — files were committed with conflict
  markers; check out the branch, resolve them, amend, and push.

The run is idempotent: it skips when `UPSTREAM_VERSION` already matches, or
when the `sync/<tag>` branch already exists.

### Reading the logs

- Full logs for every run:
  [Actions](https://github.com/aayushpokhrel49/Aaykra/actions). Filter by
  workflow, and use **Re-run failed jobs** to retry only what broke.
- The bundle steps upload their artifact with `if-no-files-found: error`, so a
  green run always has a downloadable artifact and a failure is never a silently
  empty one.
- `build-deb` runs `dpkg-deb --info` and `--contents` on the package it built, so
  the manifest and file list are in the log without unpacking anything.
- The upstream sync run prints `sync.log` into the pull request body, so the
  conflict list is readable without opening the Actions log.

### Cutting a release

```sh
# Add your entries under "## Unreleased" in .github/release/CHANGELOG.md first.
script/release 1.0.90              # bump version, date the changelog, commit, tag, push
script/release 1.0.90 --no-push    # same, but stop before pushing
```

Pushing the `v*` tag starts the release workflow. Watch it at
[actions/workflows/release.yml](https://github.com/aayushpokhrel49/Aaykra/actions/workflows/release.yml).

### Repository secrets

| Secret                         | Used by         | Purpose                                         |
| ------------------------------ | --------------- | ----------------------------------------------- |
| `MACOS_CERTIFICATE`            | `release`       | Developer ID certificate for signing the `.app` |
| `MACOS_CERTIFICATE_PASSWORD`   | `release`       | Password for the certificate                    |
| `MACOS_PROVISIONING_PROFILE`   | `release`       | Base64-encoded embedded provisioning profile    |
| `APPLE_NOTARIZATION_KEY`       | `release`       | App Store Connect API private key               |
| `APPLE_NOTARIZATION_KEY_ID`    | `release`       | Notarization key id                             |
| `APPLE_NOTARIZATION_ISSUER_ID` | `release`       | Notarization issuer id                          |
| `APPLE_NOTARIZATION_TEAM`      | `release`       | Notarization team id                            |
| `SYNC_TOKEN`                   | `upstream-sync` | Token used to open the sync pull request        |

Without the macOS secrets the workflow still succeeds and the bundle is ad-hoc
signed, which is why macOS Gatekeeper prompts on first launch — see
[INSTALL.md](./INSTALL.md).

## Contributing

Bug reports, feature ideas, and pull requests are welcome. Open an issue or
submit a PR on the [repository](https://github.com/aayushpokhrel49/Aaykra).

Before opening a PR:

1. Add a bullet under `## Unreleased` in `.github/release/CHANGELOG.md`
   describing your user-facing change.
2. Fill in the objective, solution, and testing sections of
   [.github/pull_request_template.md](./.github/pull_request_template.md).
3. Run `./script/clippy` and the relevant tests.

Sync PRs from `upstream-sync` follow the same template, with `- N/A` under
**Release Notes**.

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
