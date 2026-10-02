# Changelog

All notable user-facing changes to Aaykra are listed here, newest first. Add a
bullet under Unreleased with your change; the version bump commit turns that
section into the release, and the release workflow copies it into the GitHub
release body.

## Unreleased

- Added the Aaykra theme, a dark theme that is now the default for new and existing installs. It is bundled with the binary rather than fetched from the extension registry, and the default appearance is pinned to dark instead of following the system, so the app no longer changes theme with the OS. `Catppuccin Latte` is still used when light appearance is requested.
- Fixed Git commands failing when a branch or ref name also matches a path in the working tree. A branch like \`docs/rewrite\` in a repository that also has a \`docs/rewrite\` directory made git reject the revision as ambiguous, so the Git panel's History tab reported "Failed to load commit history" and commit search returned nothing. Revisions are now terminated with \`--\`, and \`--no-ext-diff\` is passed directly after the subcommand so it is not read as a pathspec.
- Fixed a diagnostic for a path outside the worktree discarding every other diagnostic in the same batch.
- Related diagnostics within a group now always link back to the primary diagnostic, instead of only when they are more than five lines away.
- Fixed merge conflicts with an empty side (no lines between the markers) producing inverted ranges, which could crash row highlighting as soon as text was typed into that side. Empty sides now use the opposite anchor biases so they grow with the text you type.
- Fixed inlay hints reported outside the buffer (an out-of-range row or a line longer than the buffer) being accepted, and made hint conversion synchronous instead of spawning a task per hint.
- Fixed case conversion dropping text after a selection that ended at a newline. A selection like "foo\nbar" with the caret at the end of the line no longer leaves "bar" behind.
- Fixed pending IME input being discarded in read-only buffers, which left a Vim `j`/`k` mapping stuck on its first keystroke.
- Fixed the deleted marker in the Git diff gutter becoming too narrow on short rows.
- Fixed code action and completion menu suffix text rendering as faded, unfocused text and covering diagnostic highlights.
- Fixed custom Markdown heading styles being applied to every heading level instead of only the one they configure. Fenced code blocks with an unknown language now also try the first word of the info string, so \`\`\`\`rust ignore\`\`\`\` highlights as Rust.
- Fixed images inside Markdown table cells ignoring the column alignment.
- Fixed data table and tabular row borders extending past the last column, and cells with absolute widths mis-sizing their rows.
- Fixed the Markdown preview tab tooltip being derived from the preview itself instead of the editor it is showing.
- The font list no longer includes the internal fallback stack or the placeholder system UI font, which stopped extension font pickers from offering non-installable fonts.
- Added a keystroke search shortcut to the keymap editor's filter bar (\`Alt-Ctrl-F\` on Linux, \`Cmd-Alt-F\` on macOS).
- Collapsing a directory in the project panel now keeps the directory selected and scrolls it into view, instead of leaving the selection on a child that no longer exists.
- Python decorators are highlighted again, including built-in \`@classmethod\`, \`@staticmethod\` and \`@property\`, and \`@\` is no longer treated as a standalone operator so matrix multiplication highlights correctly.
- The settings JSON editor handles escaped keys in nested objects, so an object key containing a quote can be renamed or removed instead of corrupting the file.
- PowerShell is now found in Scoop installs that set \`SCOOP\` to a non-default location, and falls back to \`%USERPROFILE%\\scoop\` when it is unset.

## 1.0.90 - 2026-10-01

- Fixed automatic updates on root-owned Linux installs (.deb and .pkg.tar.zst) repointing the CLI symlinks, desktop entry, and icons at the per-user copy under `~/.local`. The repointing code was compiled out in v1.0.80, so the old build kept being launched and every start asked to restart again; the new build is now the one that runs.

## 1.0.80 - 2026-09-29

- The menus are now shown in the title bar by default. Set `"title_bar": { "show_menus": false }` in your settings to go back to the single-row title bar. macOS still uses its native system menu bar.
- Added a Terminal menu to the application menu bar, with New (regular, centered, and local), Show and Focus Terminal Panel, Tasks (Run Task, Rerun Last Task, Configure Tasks, Open Global Tasks File), Rename Terminal, and the standard clipboard actions.
- Submenus nested more than one level deep in a context menu are now expanded recursively instead of being dropped, so a context menu built from a nested menu keeps all of its entries.
- Fixed automatic updates on Linux package installs (.deb and .pkg.tar.zst) never sticking: the update is staged under `~/.local` because `/opt/aaykra` is root-owned, and the CLI symlinks, desktop entry, and icons are now repointed at that copy. Previously every launch kept starting the old build and asking to restart again.

## 1.0.70 - 2026-09-27

- Renamed the editor to AAYKRA throughout: the crate, the `aaykra` binary and CLI, the `aaykra://` URL scheme, the data directories, the app menus, the About window, and every release artifact. Installs that predate the rename keep using their existing `Aayushi Code` directories, and the pre-rename `aayushicode://` links and CLI handshake still work.
- Fixed the pre-rename `.aayushicode` project-config fallback: the worktree file scanner, the debug scenario path, and the debug modal once again recognize `.aayushicode` alongside `.aaykra`.
- Renamed the in-project config folder to `.aaykra` for `settings.json`, `tasks.json`, and `debug.json`, and the remote server folders to `.aaykra_server`/`.aaykra_wsl_server`. Existing `.aaykra` (and older `.zed`) folders are still picked up as a fallback, so no project needs to be migrated by hand.
- Fixed auto-update for installs that predate the AAYKRA rebrand: each release now also publishes the legacy `aaykra-*`/`Aaykra-*` asset names, and the updater falls back to them when the `aaykra-*` asset is missing.
- Pointed every in-app documentation link (settings, themes, key bindings, tasks, debugger, git, remote development, extensions, and troubleshooting) at https://code.aayushpokhrel.info.np/docs.

## 1.0.60 - 2026-09-24

- Rebranded installers, packaging, and GitHub release artifacts to AAYKRA on every platform: macOS `.dmg`, Linux `.tar.gz`/`.deb`/`.pkg.tar.zst`, Windows `.exe`, Snap, Flatpak, and the remote-server binaries.
- Pointed the auto-updater, install scripts, and docs at the renamed Aaykra repository after moving the project on GitHub.

## 1.0.50 - 2026-09-23

- Rebranded the editor from Aaykra to AAYKRA across the UI: app menus, welcome and onboarding screens, About window, notifications, settings descriptions, theme and icon-theme names, and installer/desktop display names. Core identifiers, data directories, and protocol strings are unchanged.

## 1.0.40 - 2026-09-21

- Added a fresh new logo across the app, installers, and packaging.
- Fixed minor bugs.

## 1.0.30 - 2026-09-20

- Fixed automatic updates on Linux package installs (Arch .pkg.tar.zst and .deb): they now use the same GitHub self-updater as the tar.gz build, checking the releases API, downloading `aaykra-linux-<arch>.tar.gz`, and applying it in place. For root-owned installs the update lands in a per-user copy under `~/.local` and the app restarts from there.

## 1.0.20 - 2026-09-19

- Fixed automatic update downloads failing with "operation timed out"; downloads and update checks now retry and time out gracefully.
- Improved update notifications: once an update is downloaded, Aaykra shows an "Update Now" prompt on every launch until you apply it.
- Cleaned up the About window to show only the app version and links to the website, GitHub, X, and email.

## 1.0.10 - 2026-09-18

- Added proper credit to Zed Industries in the README, acknowledging that Aaykra is a fork of Zed and is built on top of its code.

## 1.0.9 - 2026-09-18

- Renamed the product, binary, and installer to Aaykra / aaykra. New install paths, app names, and data directories (existing Aaykra data is reused; no re-setup needed).
- Fixed macOS release builds: the Intel build now runs on the supported `macos-15-intel` runner and the Apple Silicon build on `macos-15`.
- Fixed the Arch Linux package build: the `.pkg.tar.zst` is written to an absolute path under the repo's `target/release` so packaging no longer fails.
- The release workflow can now be triggered manually ("Run workflow" with a version) in addition to tag pushes, and it publishes every platform package that builds successfully.

## 1.0.8 - 2026-09-12

- Material Icon Theme is now the default icon theme, with light and dark variants that follow the theme mode.
- Synced with Zed 1.19.2: multi-select in the Git panel, automatic language detection for untitled buffers, project search on type, recency-sorted command palette, the `reveal_if_open` setting, and many fixes.
- Removed "Delete Permanently" from the project panel context menu. Delete always moves to the Trash.
- Right-clicking the empty space below the file tree now opens the context menu.
- Tabs for files that no longer exist show "File not found. It was deleted or moved." without offering to recreate the file.
- Release builds compile every crate as a single codegen unit again for better runtime performance.

## 1.0.7 - 2026-09-11

- Files that were deleted while Aaykra was closed reopen as strikethrough tabs with a "file not found" message instead of a blank editor.
- Tabs show the file's icon before its name.
- The project panel's Delete action moves files to the Trash, with a separate Delete Permanently option.
- Activity bar icons match VS Code's, with more vertical spacing, and all left-dock panels share one width.
- Search moved into its own panel, and activity bar items can be reordered.
- The file tree scrolls a little past its last entry.
