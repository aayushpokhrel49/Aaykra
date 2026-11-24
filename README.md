<p align="center">English | <a href="README.zh-CN.md">中文</a></p>

<div align="center">
  <img src="crates/aaykra/resources/app-icon.png" alt="Aaykra" width="128">
  <h1>Aaykra</h1>
  <p>The fast, native code editor and agent workspace.</p>
  <p>Written in Rust. Batteries included.</p>
  <p><a href="#download"><strong>Download</strong></a></p>
</div>

---

Aaykra is a fast, native code editor written in Rust, with a clean, modern design and coding agents built in. Its name reflects a focus on flow and effortless action. Aaykra is a fork of [Zed](https://github.com/zed-industries/zed) but diverges heavily from it.

## Features

- ⚡ **Native and fast.** No Electron or webviews. Renders on the GPU.
- 🍃 **Lightweight.** Lower memory footprint than VS Code and Zed. See [benchmarks](https://github.com/aayushpokhrel49/aaykra/blob/main/docs/memory-benchmark.md).
- 👾 **Coding agents included.** Built-in support for Claude Code, Codex, OpenCode, and Antigravity.
- ✨ **Beautiful by default.** Clean, modern look with a delightful UX.
- 🤫 **Completely private.** No account or telemetry. Aaykra never sends your usage data anywhere.

## Download

On macOS and Linux, run:

```sh
curl -fsSL https://code.aayushpokhrel.com.np/install.sh | sh
```

For Windows, download the [installer](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/Aaykra-x86_64.exe).

<details>
<summary>Or install manually</summary>

- [macOS (Apple Silicon)](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/Aaykra-aarch64.dmg)
- [macOS (Intel)](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/Aaykra-x86_64.dmg)
- [Windows x86-64](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/Aaykra-x86_64.exe)
- [Linux x86-64](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/aaykra-linux-x86_64.tar.gz)
- [Linux AArch64](https://github.com/aayushpokhrel49/aaykra/releases/latest/download/aaykra-linux-aarch64.tar.gz)

See [INSTALL.md](https://github.com/aayushpokhrel49/aaykra/blob/main/docs/INSTALL.md) for more.
</details>

---

![Aaykra screenshot](https://code.aayushpokhrel.com.np/screenshot.png)

## Docs

See [docs](https://code.aayushpokhrel.com.np).

## Building

Aaykra builds the same way Zed does. See the [Zed development docs](https://zed.dev/docs/development).

## Licensing

Aaykra is licensed under GPL-3.0-or-later with Apache-2.0 components where marked. See [LICENSE-GPL](./LICENSE-GPL) and [LICENSE-APACHE](./LICENSE-APACHE).

## Acknowledgements

Thanks to the Zed team for building an excellent editor and releasing it as open source. Aaykra would not exist without their work.
