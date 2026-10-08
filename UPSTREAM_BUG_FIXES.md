# Upstream Bug Fixes

Upstream Zed bug fixes and breaking changes released since the Aaykra upstream base,
collected so they can be ported during future upstream syncs.

Checkboxes track porting progress: an entry is ticked once the upstream fix has been
applied to the Aaykra source tree. **20 of 101 entries (20%) are ported so far.**

## Scope

| | |
|---|---|
| Aaykra base | Zed `v1.19.2` (see [`UPSTREAM_VERSION`](./UPSTREAM_VERSION)) |
| Releases covered | `v1.20.1`, `v1.20.2`, `v1.21.0`, `v1.22.0`, `v1.23.1-pre` |
| Upstream source | [Zed release notes](https://github.com/zed-industries/zed/releases) |
| Bug fixes listed | 96 |
| Breaking changes listed | 5 |
| Entries excluded by policy | 38 |
| Ported so far | 20 of 101 |

There is no `v1.20.0` release; Zed went from `v1.19.2` to `v1.20.1`.

## How to use this list

- Checked entries (`- [x]`) have been ported into Aaykra; unchecked entries have not
  been verified against the Aaykra source tree.
- Tick a box once the fix has been applied to Aaykra.
- Entries are grouped by subsystem, not by release. The release tag(s) show where
  each fix first appeared upstream.
- A pull request that was cherry-picked across release channels is listed once,
  with every tag it appeared in.
- [#64515](https://github.com/zed-industries/zed/pull/64515) appears twice on
  purpose: once as a bug fix, once as a breaking change.

### Exclusions

Entries touching the following areas are deliberately left out of this document and
are listed in the [appendix](#appendix-excluded-entries) instead:

- AI, agents, assistants, and language models
- Accounts, authentication, and subscriptions
- Zed Cloud, collaboration, and shared databases
- Telemetry and data-retention consent

## Summary

| Subsystem | Entries |
|---|---|
| [Vim & Helix](#vim-and-helix) | 4 |
| [Debugger](#debugger) | 2 |
| [Markdown & Preview](#markdown-and-preview) | 6 |
| [Tables & Tabular Data](#tables-and-tabular-data) | 1 |
| [Terminal & Tasks](#terminal-and-tasks) | 6 |
| [LSP, Diagnostics & Completions](#lsp-diagnostics-and-completions) | 16 |
| [Git & Version Control](#git-and-version-control) | 13 |
| [Syntax Highlighting & Languages](#syntax-highlighting-and-languages) | 3 |
| [Extensions & WASM](#extensions-and-wasm) | 3 |
| [Remote & Projects](#remote-and-projects) | 5 |
| [Performance & Memory](#performance-and-memory) | 2 |
| [Search & File Discovery](#search-and-file-discovery) | 3 |
| [Settings & Keymap Editor](#settings-and-keymap-editor) | 5 |
| [Linux, Wayland & X11](#linux-wayland-and-x11) | 6 |
| [macOS & Windows](#macos-and-windows) | 5 |
| [Editor: Input, Selection & Navigation](#editor-input-selection-and-navigation) | 16 |
| [Breaking Changes and Notices](#breaking-changes-and-notices) | 5 |
| **Total** | **101** |

## Bug fixes

### Vim & Helix

_Vim and Helix modal editing behaviour._

- [ ] Fixed Vim's `o` not continuing the comment prefix inside C-style multiline comments. ([#63751](https://github.com/zed-industries/zed/pull/63751), `v1.20.1`)
- [ ] Fixed a crash when pasting in an expanded deleted diff hunk in Helix mode. ([#64245](https://github.com/zed-industries/zed/pull/64245), `v1.21.0`)
- [ ] Fixed Helix buffer picker opening in the wrong pane on `space b`. ([#64494](https://github.com/zed-industries/zed/pull/64494), `v1.23.1-pre`)
- [ ] Fixed a crash when pasting multiple selections at the end of a file in Vim mode. ([#64876](https://github.com/zed-industries/zed/pull/64876), `v1.23.1-pre`)

### Debugger

_Debug adapter protocol and debug scenarios._

- [ ] Fixed the debugger's "Copy Value" copying the truncated preview instead of the full value. ([#63902](https://github.com/zed-industries/zed/pull/63902), `v1.20.1`)
- [ ] Debugger: Fixed "Rerun last debug scenario" not running the last scheduled scenario. ([#64856](https://github.com/zed-industries/zed/pull/64856), `v1.23.1-pre`)

### Markdown & Preview

_Markdown preview rendering._

- [x] Fixed Markdown preview headings not rendering at their configured weight. ([#63465](https://github.com/zed-industries/zed/pull/63465), `v1.20.1`)
- [x] Fixed Markdown preview tabs not showing the source file path on hover. ([#64318](https://github.com/zed-industries/zed/pull/64318), `v1.22.0`)
- [ ] Fixed syntax highlighting for Markdown emphasis, links, and code blocks when viewing a file at a historical commit. ([#64683](https://github.com/zed-industries/zed/pull/64683), `v1.22.0`, `v1.23.1-pre`)
- [ ] Fixed a rare crash when hovering wrapped Markdown text on Linux. ([#64672](https://github.com/zed-industries/zed/pull/64672), `v1.23.1-pre`)
- [ ] Fixed search highlights in Markdown Preview that hid matched text when the theme used opaque highlight colors. ([#64788](https://github.com/zed-industries/zed/pull/64788), `v1.23.1-pre`)
- [x] Fixed images in Markdown table cells ignoring column alignment and vertical centering. ([#64914](https://github.com/zed-industries/zed/pull/64914), `v1.23.1-pre`)

### Tables & Tabular Data

_Tabular data previews._

- [x] Fixed row separator lines in tabular data previews extending past the last column. ([#63367](https://github.com/zed-industries/zed/pull/63367), `v1.23.1-pre`)

### Terminal & Tasks

_Terminal rendering, shells, task execution._

- [ ] Fixed task worktree variables resolving to the wrong project when the global `tasks.json` file was focused, and paths containing spaces breaking Windows tasks. ([#55727](https://github.com/zed-industries/zed/pull/55727), `v1.22.0`)
- [x] Fixed PowerShell installed by Scoop not being detected when Scoop uses a custom installation directory. ([#62473](https://github.com/zed-industries/zed/pull/62473), `v1.22.0`)
- [ ] Fixed the terminal grid losing its top alignment when growing on Windows. ([#63699](https://github.com/zed-industries/zed/pull/63699), `v1.22.0`)
- [ ] Fixed missing visual feedback when copying REPL output. ([#64276](https://github.com/zed-industries/zed/pull/64276), `v1.22.0`)
- [ ] Fixed terminals retaining about 2 MB of memory per command after the command finished. ([#64405](https://github.com/zed-industries/zed/pull/64405), `v1.22.0`)
- [ ] Fixed the pending keybindings list showing `task: spawn` instead of the task name for bindings that spawn a task. ([#64937](https://github.com/zed-industries/zed/pull/64937), `v1.23.1-pre`)

### LSP, Diagnostics & Completions

_Language servers, diagnostics, completions, hover, inlay hints._

- [ ] Fixed a BasedPyright memory leak caused by incorrect workspace diagnostics polling. ([#63336](https://github.com/zed-industries/zed/pull/63336), `v1.20.1`)
- [ ] Fixed diagnostics from the previous language server staying on a file after changing its language. ([#63460](https://github.com/zed-industries/zed/pull/63460), `v1.20.1`)
- [ ] Fixed CRLF line breaks not normalized in completion labels. ([#63984](https://github.com/zed-industries/zed/pull/63984), `v1.20.1`)
- [x] Fixed diagnostics batches stopping at paths without a worktree. ([#64073](https://github.com/zed-industries/zed/pull/64073), `v1.20.1`)
- [ ] Fixed a crash during Python interpreter discovery when an executable emitted non-UTF-8 output. ([#64040](https://github.com/zed-industries/zed/pull/64040), `v1.21.0`)
- [ ] Fixed a bug on macOS where moving the pointer over another app could trigger hover effects in a Zed window underneath it. ([#64234](https://github.com/zed-industries/zed/pull/64234), `v1.21.0`)
- [ ] Fixed language servers not starting for files opened via UNC paths on Windows. ([#56553](https://github.com/zed-industries/zed/pull/56553), `v1.22.0`)
- [ ] Fixed missing editor underlines for language server diagnostics at positions without underlying text. ([#63933](https://github.com/zed-industries/zed/pull/63933), `v1.22.0`)
- [ ] Fixed Command-clicking a symbol opening a definitions multibuffer instead of the picker when `lsp_results_location` was set to `"picker"`. ([#64039](https://github.com/zed-industries/zed/pull/64039), `v1.22.0`)
- [x] Fixed diagnostic navigation not linking nearby related diagnostics back to the primary diagnostic. ([#64134](https://github.com/zed-industries/zed/pull/64134), `v1.22.0`)
- [ ] Fixed LSP completions without edit ranges ignoring the configured `completions.lsp_insert_mode`. ([#64139](https://github.com/zed-industries/zed/pull/64139), `v1.22.0`)
- [ ] Fixed language server formatting scrolling to the bottom when the server replaces the entire buffer. ([#57269](https://github.com/zed-industries/zed/pull/57269), `v1.23.1-pre`)
- [ ] Fixed closing one LSP Logs view stopping streams used by another view or downstream client. ([#61222](https://github.com/zed-industries/zed/pull/61222), `v1.23.1-pre`)
- [ ] Fixed hover flicker after dismissing native menus on macOS. ([#64175](https://github.com/zed-industries/zed/pull/64175), `v1.23.1-pre`)
- [x] Fixed a bug where inlay hints positioned past the end of a buffer could appear on its last line, and inlay hints exactly at the end of a file without a trailing newline could be hidden. ([#64659](https://github.com/zed-industries/zed/pull/64659), `v1.23.1-pre`)
- [x] Fixed completion details lacking visual distinction from completion labels. ([#64684](https://github.com/zed-industries/zed/pull/64684), `v1.23.1-pre`)

### Git & Version Control

_Git Panel, diffs, gutters, worktrees, ignore rules._

- [x] Fixed the deleted git-gutter marker becoming nearly invisible when `git_gutter_width` was set to a small custom pixel value. ([#63434](https://github.com/zed-industries/zed/pull/63434), `v1.20.1`)
- [x] Fixed the Git Panel's History tab, commit search, branch diff, and opening a commit by ref failing when a branch name matched a path in the working tree. ([#63666](https://github.com/zed-industries/zed/pull/63666), `v1.20.1`)
- [ ] Fixed Git Panel keybindings matching the Changes tab while the History tab was active. ([#63689](https://github.com/zed-industries/zed/pull/63689), `v1.20.1`)
- [ ] Fixed hints for branch and worktree deletion, keybinding conflicts, and edit predictions referring to Alt instead of Option on macOS. ([#63807](https://github.com/zed-industries/zed/pull/63807), `v1.20.1`)
- [ ] Fixed Project Panel reveal in Git diff multi-buffers. ([#64154](https://github.com/zed-industries/zed/pull/64154), `v1.20.1`)
- [ ] Fixed the Git Panel unexpectedly switching repositories when viewing changes in multi-repository projects. ([#58795](https://github.com/zed-industries/zed/pull/58795), `v1.21.0`)
- [ ] Fixed a bug where deleted files appeared outside the file tree in the Outline Panel when viewing a diff. ([#63570](https://github.com/zed-industries/zed/pull/63570), `v1.21.0`)
- [ ] Fixed diff statistics to use the theme's version-control colors for added and deleted line counts. ([#64083](https://github.com/zed-industries/zed/pull/64083), `v1.21.0`)
- [ ] Fixed deleted Git gutter markers disappearing at narrow custom widths. ([#64469](https://github.com/zed-industries/zed/pull/64469), `v1.21.0`)
- [x] Fixed a crash when typing into an empty side of a merge conflict. ([#64604](https://github.com/zed-industries/zed/pull/64604), `v1.21.0`)
- [ ] Fixed excerpt expansion when the selection was on deleted text in unified diffs. ([#63946](https://github.com/zed-industries/zed/pull/63946), `v1.22.0`)
- [ ] Fixed “Add to .git/info/exclude” failing in secondary Git worktrees. ([#63967](https://github.com/zed-industries/zed/pull/63967), `v1.22.0`)
- [ ] Fixed commit details and diffs waiting behind an in-progress fetch, pull, or push before opening. ([#64720](https://github.com/zed-industries/zed/pull/64720), `v1.23.1-pre`)

### Syntax Highlighting & Languages

_Tree-sitter grammars and syntax highlighting._

- [ ] Fixed syntax highlighting for injected languages in strings containing interpolations. ([#49265](https://github.com/zed-industries/zed/pull/49265), `v1.20.1`)
- [x] Fixed Python decorator syntax highlighting conflicting with the matrix multiplication operator. ([#58077](https://github.com/zed-industries/zed/pull/58077), `v1.21.0`)
- [x] Fixed syntax highlighting for fenced code blocks with extra text after the language name. ([#64694](https://github.com/zed-industries/zed/pull/64694), `v1.23.1-pre`)

### Extensions & WASM

_Extension toolchain, extension host, snippets._

- [ ] Fixed Node tool installations failing or completing partially when packages listed native bindings as optional dependencies. ([#52451](https://github.com/zed-industries/zed/pull/52451), `v1.20.1`)
- [ ] Fixed compilation of development extensions on Windows ARM64. ([#63816](https://github.com/zed-industries/zed/pull/63816), `v1.21.0`)
- [ ] Fixed snippets from different extensions canceling each other out for the same language. ([#58659](https://github.com/zed-industries/zed/pull/58659), `v1.22.0`)

### Remote & Projects

_Remote servers and project roots._

- [ ] Fixed the remote projects picker not closing when opening a folder on a remote server. ([#63628](https://github.com/zed-industries/zed/pull/63628), `v1.20.1`)
- [ ] Fixed an issue in SSH remote mode where reopening a newly created file at a path that was previously renamed could incorrectly reuse the buffer for the renamed file. ([#64028](https://github.com/zed-industries/zed/pull/64028), `v1.20.1`)
- [ ] Fixed remote server removal prompts not capturing keyboard focus. ([#60965](https://github.com/zed-industries/zed/pull/60965), `v1.21.0`)
- [ ] Fixed a bug where folders remained highlighted after they stopped being dragged. ([#64038](https://github.com/zed-industries/zed/pull/64038), `v1.21.0`)
- [ ] Fixed a bug where a project root folder could not be renamed to match a child folder. ([#64268](https://github.com/zed-industries/zed/pull/64268), `v1.21.0`)

### Performance & Memory

_Leaks and retained memory._

- [ ] Fixed intermittent "database is locked" errors when sharing a database across Zed instances. ([#63923](https://github.com/zed-industries/zed/pull/63923), `v1.21.0`)
- [ ] Fixed a memory leak when closing macOS windows with experimental accessibility enabled. ([#64143](https://github.com/zed-industries/zed/pull/64143), `v1.22.0`)

### Search & File Discovery

_File patterns, exclusions, globs._

- [x] Fixed the Keymap Editor's "Search by Keystrokes" shortcut, `cmd-alt-f` (macOS) and `ctrl-alt-f` (Linux/Windows), being shadowed by the file finder. ([#63376](https://github.com/zed-industries/zed/pull/63376), `v1.20.1`)
- [ ] Fixed importing VS Code `files.exclude` into `file_scan_exclusions`. ([#64492](https://github.com/zed-industries/zed/pull/64492), `v1.22.0`)
- [ ] Fixed invalid file patterns discarding valid exclusion, hidden-file, read-only, and private-file rules or causing inclusion and file-type settings to panic. ([#64525](https://github.com/zed-industries/zed/pull/64525), `v1.22.0`)

### Settings & Keymap Editor

_Settings UI, settings files, keymap editor, font suggestions._

- [x] Fixed a settings key containing a quote or a backslash corrupting `settings.json`. ([#62949](https://github.com/zed-industries/zed/pull/62949), `v1.20.1`)
- [x] Fixed font suggestions listing unavailable fallback fonts and internal font aliases. ([#64095](https://github.com/zed-industries/zed/pull/64095), `v1.21.0`)
- [ ] Fixed the Settings window opening with its header off-screen at high DPI scaling on Windows. ([#62073](https://github.com/zed-industries/zed/pull/62073), `v1.22.0`)
- [ ] Fixed recording and searching for standalone modifier-key bindings in the Keymap Editor. ([#64537](https://github.com/zed-industries/zed/pull/64537), `v1.22.0`)
- [ ] Fixed a bug where “Configure Excluded Files” selected the wrong list when a commented-out `disabled_globs` setting appeared before it. ([#64540](https://github.com/zed-industries/zed/pull/64540), `v1.22.0`)

### Linux, Wayland & X11

_Linux-specific behaviour._

- [ ] Fixed IME staying enabled in the Project Panel on Wayland when no text input was focused. ([#63776](https://github.com/zed-industries/zed/pull/63776), `v1.20.1`)
- [ ] Fixed Linux `mailto:` URI handling when using `Help → Email Us...`. ([#64090](https://github.com/zed-industries/zed/pull/64090), `v1.20.1`)
- [ ] Fixed a Linux startup crash when local XKB keyboard-definition files were unavailable. ([#64113](https://github.com/zed-industries/zed/pull/64113), `v1.21.0`)
- [ ] Fixed canceled external file drags on Linux Wayland sometimes remaining active and causing later clicks to copy the dragged file. ([#64122](https://github.com/zed-industries/zed/pull/64122), `v1.21.0`)
- [ ] Fixed `cmd-w` (macOS) and `ctrl-w` (Linux/Windows) closing the active dock instead of the active pane item. ([#64515](https://github.com/zed-industries/zed/pull/64515), `v1.22.0`)
- [ ] Fixed a crash on Linux (X11) when a window was redrawn immediately after GPU device recovery. ([#64570](https://github.com/zed-industries/zed/pull/64570), `v1.22.0`)

### macOS & Windows

_macOS and Windows specific behaviour._

- [ ] Fixed double-clicking the title bar ignoring the macOS "Tiled windows have margins" setting when "Double-click a window's title bar" was set to "Fill". ([#63759](https://github.com/zed-industries/zed/pull/63759), `v1.20.1`)
- [ ] Fixed unintended whole-canvas selection when long-pressing in GPUI web applications on iOS. ([#63877](https://github.com/zed-industries/zed/pull/63877), `v1.20.1`)
- [ ] Fixed a bug where restored macOS windows reopened on the currently active Space instead of their original Space. ([#58886](https://github.com/zed-industries/zed/pull/58886), `v1.21.0`)
- [ ] Fixed the macOS traffic light animation when exiting fullscreen. ([#64339](https://github.com/zed-industries/zed/pull/64339), `v1.21.0`)
- [ ] Fixed ligatures being disabled on Windows unless `buffer_font_features` was set. ([#62338](https://github.com/zed-industries/zed/pull/62338), `v1.22.0`)

### Editor: Input, Selection & Navigation

_Cursor, selections, soft wrap, editing, key handling, layout._

- [x] Fixed case conversion commands deleting the line break when the selection ended at the start of the next line. ([#63463](https://github.com/zed-industries/zed/pull/63463), `v1.20.1`)
- [ ] Fixed the `preview_tabs.enable_preview_from_project_panel` setting being ignored when opening files from the Project Panel with the keyboard. ([#63758](https://github.com/zed-industries/zed/pull/63758), `v1.20.1`)
- [ ] Fixed the gutter tooltip's modifier-click hint after holding Command. ([#64124](https://github.com/zed-industries/zed/pull/64124), `v1.21.0`)
- [x] Fixed the Project Panel failing to scroll to collapsed parent folders. ([#64207](https://github.com/zed-industries/zed/pull/64207), `v1.21.0`)
- [ ] Fixed columnar selections landing in the wrong column around non-ASCII text, tabs, soft wraps, and collapsed content. ([#63723](https://github.com/zed-industries/zed/pull/63723), `v1.22.0`)
- [ ] Fixed the cursor drifting from the center of the editor when using a high `vertical_scroll_margin` for typewriter-style scrolling. ([#63944](https://github.com/zed-industries/zed/pull/63944), `v1.22.0`)
- [ ] Fixed cursor animations when moving between characters of different widths and during keyboard-triggered autoscrolling. ([#64184](https://github.com/zed-industries/zed/pull/64184), `v1.22.0`)
- [ ] Fixed a bug where the block cursor hid the character underneath until its animation completed. ([#64521](https://github.com/zed-industries/zed/pull/64521), `v1.22.0`)
- [ ] Fixed Project Panel requiring two `left` presses to collapse opened files’ parent. ([#64733](https://github.com/zed-industries/zed/pull/64733), `v1.22.0`, `v1.23.1-pre`)
- [ ] Fixed breadcrumb highlighting outside excerpts. ([#64738](https://github.com/zed-industries/zed/pull/64738), `v1.22.0`, `v1.23.1-pre`)
- [ ] Fixed soft-wrap continuation line indentation calculation for unindented lines starting at column 0. ([#63468](https://github.com/zed-industries/zed/pull/63468), `v1.23.1-pre`)
- [ ] Fixed centered layout not applying when a pane was zoomed in. ([#64064](https://github.com/zed-industries/zed/pull/64064), `v1.23.1-pre`)
- [ ] Fixed delayed selection of the first entry when opening a context menu. ([#64365](https://github.com/zed-industries/zed/pull/64365), `v1.23.1-pre`)
- [ ] Fixed indentation corruption during multicursor editing and block indentation. ([#64675](https://github.com/zed-industries/zed/pull/64675), `v1.23.1-pre`)
- [x] Fixed a crash when typing the start of a multi-key binding at the end of a read-only file. ([#64730](https://github.com/zed-industries/zed/pull/64730), `v1.23.1-pre`)
- [ ] Fixed entries folding when clicked in the Outline Panel. ([#64736](https://github.com/zed-industries/zed/pull/64736), `v1.23.1-pre`)

### Breaking Changes and Notices

_Changes that alter existing behaviour or settings when porting._

- [ ] Moved the Markdown Preview font settings under the `markdown_preview` key as `font_size`, `font_family` and `code_font_family`. Existing settings are migrated automatically. ([#63462](https://github.com/zed-industries/zed/pull/63462), `v1.20.1`)
- [ ] Stopped importing VS Code’s `files.watcherInclude` as `file_scan_inclusions`; watch roots are not Git-ignore overrides. ([#64440](https://github.com/zed-industries/zed/pull/64440), `v1.22.0`)
- [ ] Changed the shortcut for closing the active dock from `cmd-w` (macOS) or `ctrl-w` (Linux/Windows) to `ctrl-alt-w` on all platforms. ([#64515](https://github.com/zed-industries/zed/pull/64515), `v1.22.0`)
- [ ] Changed `terminal.path_hyperlink_regexes` so a top-level `"..."` entry now inserts inherited regexes. To retain the previous three-character regex, replace that entry with `["..."]`. ([#64552](https://github.com/zed-industries/zed/pull/64552), `v1.22.0`)
- [ ] Changed title-bar application menus on Linux and Windows to require a click before opening. Enable `title_bar.open_menus_on_hover` to restore opening on hover. ([#48496](https://github.com/zed-industries/zed/pull/48496), `v1.23.1-pre`)

## Appendix: excluded entries

These upstream entries were filtered out. They are listed here so the filtering is
auditable rather than invisible — reopen an entry here only if Aaykra decides it needs
that feature.

### AI & language models (29)

_Touches AI features, agents, assistants, or language models._

- Fixed the Agent Panel appearing in the "View" menu when AI was disabled. ([#63580](https://github.com/zed-industries/zed/pull/63580), `v1.20.1`)
- Fixed `agent: manage skills` appearing in the command palette when AI was disabled. ([#63598](https://github.com/zed-industries/zed/pull/63598), `v1.20.1`)
- Fixed long `ask_user` options being truncated instead of wrapping in the Agent Panel. ([#63656](https://github.com/zed-industries/zed/pull/63656), `v1.20.1`)
- Fixed ChatGPT subscription usage limits being reported as temporary OpenAI request throttling. ([#63738](https://github.com/zed-industries/zed/pull/63738), `v1.20.1`)
- Fixed projects leaking via the Agent Panel. ([#63765](https://github.com/zed-industries/zed/pull/63765), `v1.20.1`)
- Fixed valid multibyte Agent Skill descriptions being rejected for exceeding a byte-based length limit. ([#63766](https://github.com/zed-industries/zed/pull/63766), `v1.20.1`)
- Changed the Agent Panel to copy plain text with `cmd-c` (macOS) and `ctrl-c` (Linux/Windows) instead of Markdown. Copying as Markdown moved to the context menu, and the `markdown::CopyAsMarkdown` action can still be bound. ([#63884](https://github.com/zed-industries/zed/pull/63884), `v1.20.1`)
- Fixed the thinking toggle for Mistral Small and Medium. ([#63535](https://github.com/zed-industries/zed/pull/63535), `v1.21.0`)
- Fixed the Inline Assistant failing to select an available fallback model when no default model was configured. ([#63963](https://github.com/zed-industries/zed/pull/63963), `v1.21.0`)
- Fixed Anthropic credit exhaustion being classified as a malformed request instead of a payment issue. ([#63988](https://github.com/zed-industries/zed/pull/63988), `v1.21.0`)
- Fixed Ollama being unable to access images returned by tool calls. ([#64121](https://github.com/zed-industries/zed/pull/64121), `v1.21.0`)
- Fixed terminal tool output in the Agent Panel to consistently use the theme's `terminal.background` color. ([#64163](https://github.com/zed-industries/zed/pull/64163), `v1.21.0`)
- Fixed auto-compaction thresholds for GitHub Copilot models with a prompt limit below their context window. ([#64195](https://github.com/zed-industries/zed/pull/64195), `v1.21.0`)
- Fixed newly available ChatGPT subscription models not appearing in the model picker. ([#64625](https://github.com/zed-industries/zed/pull/64625), `v1.21.0`)
- Fixed a bug where the Agent Panel created two terminals when starting a new terminal thread in a project with no existing terminals. ([#62151](https://github.com/zed-industries/zed/pull/62151), `v1.22.0`)
- Fixed keyboard navigation and submission in ACP elicitation forms. ([#64193](https://github.com/zed-industries/zed/pull/64193), `v1.22.0`)
- Changed Threads Sidebar settings to `agent.threads_sidebar.position`, `agent.threads_sidebar.default_width`, and `agent.threads_sidebar.auto_open`, grouped together in the Settings Editor. Existing settings are migrated automatically. ([#64395](https://github.com/zed-industries/zed/pull/64395), `v1.22.0`)
- Fixed errors when ACP agents returned `null` for empty response payloads. ([#64457](https://github.com/zed-industries/zed/pull/64457), `v1.22.0`)
- Fixed external-agent terminals completing prematurely or reporting the wrong exit status. ([#64544](https://github.com/zed-industries/zed/pull/64544), `v1.22.0`)
- Fixed adding the current line to an Agent thread when no text was selected using `cmd->` (macOS), `ctrl->` (Linux), or `ctrl-shift-.` (Windows). ([#64589](https://github.com/zed-industries/zed/pull/64589), `v1.22.0`)
- Fixed remote external-agent exit errors appearing before the agent's final response updates were applied. ([#64603](https://github.com/zed-industries/zed/pull/64603), `v1.22.0`)
- Fixed terminal tool call output covering the rounded corners of its card in the Agent Panel. ([#64328](https://github.com/zed-industries/zed/pull/64328), `v1.23.1-pre`)
- Fixed missing or outdated output in agent tool results. ([#64708](https://github.com/zed-industries/zed/pull/64708), `v1.23.1-pre`)
- Fixed external-agent sessions remaining loaded after their conversation was closed during loading. ([#64717](https://github.com/zed-industries/zed/pull/64717), `v1.23.1-pre`)
- ACP: Fixed partial tool-call display updates when content referenced an unavailable terminal. ([#64784](https://github.com/zed-industries/zed/pull/64784), `v1.23.1-pre`)
- Fixed wrap guides showing in the Agent Panel before a conversation started. ([#64886](https://github.com/zed-industries/zed/pull/64886), `v1.23.1-pre`)
- Fixed agent notifications showing a thread's original title after the thread was renamed. ([#64907](https://github.com/zed-industries/zed/pull/64907), `v1.23.1-pre`)
- Fixed stale agent responses that could clear a newer prompt or show errors from an earlier send. ([#64917](https://github.com/zed-industries/zed/pull/64917), `v1.23.1-pre`)
- Fixed issues where OpenAI model tool calls to MCP servers treated optional fields as required, causing errors. ([#64920](https://github.com/zed-industries/zed/pull/64920), `v1.23.1-pre`)

### Cloud & collaboration (6)

_Touches Zed Cloud, collaboration, or shared databases._

- Fixed collab language server requests cancelling each other. ([#63736](https://github.com/zed-industries/zed/pull/63736), `v1.20.1`)
- Fixed private files being shared with collaborators through project search. ([#63860](https://github.com/zed-industries/zed/pull/63860), `v1.20.1`)
- Fixed the selection highlight flickering in the Threads Sidebar when clicking terminal threads. ([#64387](https://github.com/zed-industries/zed/pull/64387), `v1.22.0`)
- Fixed incorrect selected and hover colors for threads in the Threads Sidebar. ([#62597](https://github.com/zed-industries/zed/pull/62597), `v1.23.1-pre`)
- Fixed the Zed Cloud connection failing with `UnknownIssuer` behind corporate TLS-inspecting proxies whose root CA is installed in the system trust store. ([#63686](https://github.com/zed-industries/zed/pull/63686), `v1.23.1-pre`)
- Fixed window controls shifting by one pixel when the Threads Sidebar was open on Linux. ([#64381](https://github.com/zed-industries/zed/pull/64381), `v1.23.1-pre`)

### Accounts & authentication (2)

_Touches accounts, authentication, or subscriptions._

- Fixed the "Organization" section rendering empty in the title bar menu when signed out. ([#63728](https://github.com/zed-industries/zed/pull/63728), `v1.20.1`)
- Fixed a crash on Windows when reading a stored credential that has no username. ([#64735](https://github.com/zed-industries/zed/pull/64735), `v1.23.1-pre`)

### Telemetry & data retention (1)

_Touches telemetry or data-retention consent._

- Fixed data-retention consent checks for hosted counting and compaction requests. ([#64194](https://github.com/zed-industries/zed/pull/64194), `v1.21.0`)
