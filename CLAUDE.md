# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

RunTerm is a Windows desktop app (Tauri 2 + React/TypeScript + Rust) that lets users compose split-pane layouts, bind them to WSL projects, and launch them as a new Windows Terminal window. The UI, error messages and user-facing strings are in **French** — keep new user-facing text in French. Everything developer-facing is in **English**: README, `docs/`, code comments, script output and commit messages.

## Commands

Frontend (Node 22.12+):

```bash
npm ci
npm run dev              # browser preview at http://127.0.0.1:1420 (localStorage, launch disabled)
npm run check            # tsc -b + vite build + vitest run
npm test                 # vitest only (jsdom)
npx vitest run src/model.test.ts -t "name"   # single test file / test
npm run format           # prettier (src, vite.config.ts, package.json, tsconfig.json)
npm run format:check
```

Rust (toolchain pinned to 1.94.0 via `rust-toolchain.toml`):

```bash
cargo test -p runterm-core
cargo test -p runterm-core <test_name>        # single test
cargo clippy -p runterm-core --all-targets -- -D warnings
cargo fmt --all --check
```

The workspace `default-members` is only `crates/core`. The `src-tauri` crate (`runterm`) has all its dependencies under `cfg(windows)` and prints a message on non-Windows; it can only be built/tested on Windows (`cargo check -p runterm`, `cargo test -p runterm`, `npm run tauri -- dev|build`). From WSL, type-check only with `cargo check -p runterm --target x86_64-pc-windows-msvc` (after `rustup target add`). To build the Windows exe + NSIS installer from WSL, run `scripts/build-windows.sh`: it rsyncs the sources to `%USERPROFILE%\runterm-build` (override with `RUNTERM_WIN_BUILD_DIR`), runs `scripts\build-windows.cmd` there via `cmd.exe` (vcvars through vswhere, portable `node\`/`toolchain\` if present), and copies `RunTerm.exe`, the setup and `SHA256SUMS.txt` into `artifacts/`. Do not share `node_modules`/`target` between Linux and Windows. CI (`.github/workflows/checks.yml`, branch pushes and PRs) runs the Linux checks above plus a Windows job that builds the NSIS installer. Releases: `scripts/release.sh X.Y.Z` bumps the version in `package.json`/`package-lock.json`, `tauri.conf.json`, both `Cargo.toml` and `Cargo.lock`, commits and tags `vX.Y.Z`; pushing the tag runs `.github/workflows/release.yml`, which checks tag/version consistency, builds on Windows and publishes the GitHub release (plus `.sig` and `latest.json` when updater artifacts are produced).

## Architecture

Three layers, with the data model defined twice (TS and Rust) and kept in sync by hand:

- **`src/`** (React UI): `model.ts` defines `Layout` (a binary tree of `pane` / `split` nodes tagged by `kind`), `Template`, `Project`, `Config`, tree helpers, `effectivePane` (template pane + project override) and `validateConfig`. `api.ts` is the only bridge to the backend: it calls Tauri commands when `isTauri()` and falls back to `localStorage` (`runterm-preview-v1`) in the browser preview. `App.tsx` holds the whole editor UI.
- **`crates/core/`** (`runterm-core`, platform-independent, fully unit-tested in `tests.rs`): serde mirror of the same schema (camelCase, `kind` tag, versioned `schema_version`), `Config::validate`/`resolve` (merges project `overrides` keyed by pane id over template panes), atomic `load`/`save`, `shell_quote`, `pane_script` (generates the per-pane Bash script from the pane's command list), and `terminal_args` (compiles the layout tree into `wt.exe` split-pane arguments).
- **`src-tauri/src/desktop.rs`** (Windows-only): Tauri commands `load_config`, `save_config`, `list_distributions`, `launch_project`. Launch flow: resolve project → check `wt.exe` and WSL distribution → verify each pane directory in WSL → `mktemp -d /tmp/runterm-*` with `umask 077` → write one script per pane via stdin (each script deletes itself and tries to rmdir the folder on start) → on any preparation error remove the folder → run `wt.exe` with args from `terminal_args`.

Invariants to preserve when changing code:

- Schema changes must be made in both `src/model.ts` and `crates/core/src/lib.rs`, including validation (limits: max 16 panes, split ratios 10–90%). A config with an unknown version or that fails to parse must never be overwritten.
- Programs are always executed with separate argument vectors (`execute`/`wsl` helpers); user commands never pass through `cmd.exe`, and values interpolated into Bash go through `shell_quote` or positional `$1` arguments.
- Config is stored at `%APPDATA%\dev.runterm.desktop\config.json`; saves are atomic and serialized via the `Storage` mutex.
- `windows_terminal_smoke` in `desktop.rs` is `#[ignore]`d — it opens a real Terminal window and needs an interactive Windows session. Manual validation steps are in `docs/windows-validation.md`.
