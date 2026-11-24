# Aaykra vs. Zed vs. VS Code: memory benchmarks

Aaykra used less settled process memory than Zed and VS Code in every measured stage of this comparison:

1. **Zed:** Aaykra used 9.0–10.0% less at idle and 6.7–58.2% less in the workspace workflow.
2. **VS Code:** Aaykra used 48% less at idle and 14.5–38.6% less in the workspace workflow.

Tested using the official Linux ARM64 releases of [Aaykra 1.0.10](https://github.com/aayushpokhrel49/aaykra/releases/tag/v1.0.10), [Zed 1.21.0](https://github.com/zed-industries/zed/releases/tag/v1.21.0), and [VS Code 1.139.1](https://code.visualstudio.com/updates/v1_139).

> These are software-rendered Linux VPS results. All editors ran real graphical interfaces through Xvfb and Mesa llvmpipe, which renders on the CPU. The numbers are not estimates of memory use on a hardware-GPU desktop.

## Results

All values are **MiB of total process-tree PSS**, where lower is better. Totals include the editor and its running child processes. Each value is the median of three independent runs' settled-memory medians.

### Idle

| Scenario | VS Code | Zed | Aaykra | Less than VS Code | Less than Zed |
| --- | ---: | ---: | ---: | ---: | ---: |
| Welcome window | 743.5 | 420.4 | 382.5 | 48.5% | 9.0% |
| Repository open, no source file open | 725.2 | 423.7 | 381.2 | 47.4% | 10.0% |

### Workspace workflow

A snapshot of Aaykra's source repository, with language servers disabled to measure editor operations separately.

| Stage | VS Code | Zed | Aaykra | Less than VS Code | Less than Zed |
| --- | ---: | ---: | ---: | ---: | ---: |
| Open 20 source files | 826.2 | 554.3 | 516.9 | 37.4% | 6.7% |
| Edit, save, and scroll | 797.6 | 557.2 | 518.8 | 35.0% | 6.9% |
| Search the repository | 823.6 | 1395.3 | 582.9 | 29.2% | 58.2% |
| Navigate and search a large log | 1326.3 | 1633.9 | 832.7 | 37.2% | 49.0% |
| Generate terminal output | 1405.5 | 1664.4 | 862.8 | 38.6% | 48.2% |
| Close tabs, retaining project state | 1007.1 | 1631.0 | 860.9 | 14.5% | 47.2% |

The workflow performs 20 verified edit/save cycles, three targeted repository searches, navigation and search in a 200,000-line log (21.9 MiB), and 20,000 lines of integrated-terminal output. The terminal retains 10,000 lines of scrollback.

## How we measured

- **Host:** Ubuntu 26.04 ARM64 VM, 6 Neoverse-N1 vCPUs, 7.7 GiB RAM, 1280 × 800 virtual display, no GPU.
- **Isolation:** Fresh profiles and project copies for active runs. Automatic updates and telemetry disabled, no accounts signed in.
- **Sampling:** `/proc/PID/smaps_rollup` across each editor's process tree. Active runs: 10 s warmup, ~0.5 s sampling during workflows, then 5 s settle and 6 samples per stage. Idle runs: 30 s settle, then 10 samples at 1s.
- **Coverage:** 9 active sessions (54 stages, 2,435 samples) and 18 idle sessions (180 samples). Zero swap PSS in all measured processes.
- **Validation:** Matching input-file hashes, saved-edit checks, terminal completion, per-process accounting checks, and window screenshots.

## Scope and limitations

Software-rendering allocations contribute to these results. Hardware-GPU desktops, other operating systems, larger dependency graphs, signed-in AI features, debugging, and long editing sessions may behave differently.

## Reproduce

See the [benchmark runner](../script/memory-benchmark-linux.py). It supports Linux and requires Python 3.11+, Xvfb, `xauth`, `dbus-run-session`, `xdotool`, `xwininfo`, `xclip`, ImageMagick, and a working Vulkan driver.
