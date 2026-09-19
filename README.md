# RunTerm

A small Windows app to compose, save and launch workspaces in **Windows Terminal + WSL**.

React/TypeScript UI, Tauri 2 app and Rust engine. No server or account required. The user interface is in French.

## Ready-to-run Windows build

Compiled files from the latest local build are in `artifacts/` (not tracked by git):

- `RunTerm_0.1.0_x64-setup.exe`: Windows x64 installer.
- `RunTerm.exe`: standalone executable, usable when WebView2 is already installed.
- `SHA256SUMS.txt`: checksums of both files.

Run these files from Windows. The binary is built and tested; it is not signed. The verification report is in [docs/validation.md](docs/validation.md).

## Usage

1. In **Modèles de layout** (layout templates), pick the bundled developer template or create your own.
2. Select a pane, split it left/right or top/bottom and drag the dividers. Arrow keys also adjust a selected divider.
3. Set the pane name, its relative directory (`.`, `backend`, `frontend`…) and its Bash actions, in the desired order.
4. In **Projets** (projects), create a project and set its absolute Linux root, its WSL distribution and its template. The **Windows Terminal profile** (name or GUID, optional) gives the panes their colors and font; when empty, RunTerm uses the profile named after the distribution.
5. Customize commands or directories for this project if needed. **Revenir aux valeurs du modèle** (reset to template values) restores the pane's inheritance.
6. **Lancer le projet** (launch project) saves the configuration, checks the directories, then opens a new Windows Terminal window.

The bundled template matches the example: Codex and Claude on top, a free shell, a Uvicorn backend and a frontend shell at the bottom. The tools must be installed in the WSL distribution; RunTerm does not install them.

Each pane has its own shell. Its actions share directory and environment changes. An interactive command or a server blocks the following actions until it exits. A failure or Ctrl+C stops the sequence and leaves the prompt available. `exit` deliberately closes the shell. Panes start independently: there is no waiting mechanism between services.

Editing a template affects its projects on the next launch; customized fields keep priority. Changing a project's template resets its customizations. A template in use cannot be deleted. Limits: 16 panes, with ratios from 10 to 90%; Windows Terminal's minimum size may limit very dense layouts.

## Windows development

Prerequisites:

- Windows 10/11 with Windows Terminal (`wt.exe` on the PATH), WSL and a distribution with Bash.
- Node.js 22.12 or later and npm.
- Rust via rustup; `rust-toolchain.toml` selects Rust 1.94.0.
- Visual Studio Build Tools with **Desktop development with C++** and the Windows SDK.
- Microsoft Edge WebView2 Runtime.

See the [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

In **Windows PowerShell**, from a checkout on a Windows drive (for example `C:\dev\runterm`):

```powershell
npm ci
npm run tauri -- dev
```

Do not share `node_modules` or `target` between Linux and Windows installs. The project can be developed under WSL, but the native build and the installer are produced with the Windows tools.

## Building the installer

```powershell
npm run tauri -- build
```

From WSL, `scripts/build-windows.sh` does the same with the Windows tools: it copies the sources to `%USERPROFILE%\runterm-build` (override with `RUNTERM_WIN_BUILD_DIR`), runs `scripts\build-windows.cmd` there, then puts `RunTerm.exe`, the installer and `SHA256SUMS.txt` into `artifacts/`. Visual Studio Build Tools are still required on the Windows side; Node and Rust can be installed on Windows or provided as portable versions in the `node\` and `toolchain\` folders of the build directory.

The NSIS installer is written to `target\release\bundle\nsis\`. It is not signed; no distribution signing is configured. The Tauri configuration installs WebView2 if needed. Windows Terminal and WSL remain prerequisites to install separately.

The **Checks and Windows installer** GitHub Actions workflow builds the installer and keeps it as a workflow artifact, without publishing a release. It runs on branch pushes, pull request or manually.

## Releasing

Releases are published by the **Release** workflow when a `v*` tag is pushed:

```bash
scripts/release.sh 0.2.0        # bumps the version in every manifest, commits "Release v0.2.0" and tags v0.2.0
git push origin main v0.2.0
```

The workflow checks that the tag matches the version in `package.json`, `src-tauri/tauri.conf.json` and both `Cargo.toml` files, runs the tests, builds on Windows and creates the GitHub release with the installer, `RunTerm.exe` and `SHA256SUMS.txt`, plus release notes generated from the commits. A tag with a suffix (`v0.2.0-beta.1`) is published as a prerelease.

The installer is signed for the updater with the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets, and the release also gets the `.sig` file and `latest.json`.

## Automatic updates

RunTerm uses the Tauri updater plugin. At startup, and from the refresh button at the bottom of the sidebar, it reads `https://github.com/mmyumu/runterm/releases/latest/download/latest.json`. When a newer version exists, a banner offers to install it: the app downloads the installer, checks its signature against the public key in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`), then runs it in passive mode and restarts. Installing is disabled while there are unsaved changes, because the installer closes the app. A failed check at startup (offline, for example) stays silent.

Setting up the signing key (once):

```bash
npx tauri signer generate -w ~/.tauri/runterm.key     # asks for a password
gh secret set TAURI_SIGNING_PRIVATE_KEY -R mmyumu/runterm < ~/.tauri/runterm.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD -R mmyumu/runterm   # prompts for the password
```

Then put the content of `~/.tauri/runterm.key.pub` into `plugins.updater.pubkey`. Back up the private key and its password: installed versions only accept updates signed with that key, so losing it means users have to reinstall manually.

Builds without the key (`checks.yml`, `scripts/build-windows.cmd` when `TAURI_SIGNING_PRIVATE_KEY` is not set) pass `--no-sign`; their installers work but cannot be served as updates.

## UI preview under WSL/Linux

```bash
npm ci
npm run dev
```

Open `http://localhost:1420`. The preview saves to the browser's local storage and disables launching. Its data is not shared with the Windows app.

## Checks

```bash
npm run check
npm run format:check
cargo test -p runterm-core
cargo clippy -p runterm-core --all-targets -- -D warnings
cargo fmt --all --check
```

On Windows, also run `cargo check -p runterm`. From WSL, use `scripts/build-windows.sh`: a Windows-targeted `cargo check` from Linux fails because a dependency of the updater needs the MSVC tools. Neither replaces actually testing Windows Terminal. The manual test plan is in [docs/windows-validation.md](docs/windows-validation.md).

## Data and architecture

- `src/`: editor, projects, templates and Tauri command client. The browser preview uses separate storage.
- `crates/core/`: versioned JSON schema, validation, resolution of customizations, atomic save, Bash scripts and compilation of the layout into Windows Terminal arguments.
- `src-tauri/`: Windows integration, WSL discovery and program execution with separate arguments. No user command goes through `cmd.exe`.

The app saves to `%APPDATA%\dev.runterm.desktop\config.json`. A corrupted configuration or one with an unknown version is never overwritten: the file is kept. Closing a window with unsaved changes asks whether to save or discard them.

Temporary scripts are created with private permissions in `/tmp/runterm-*` of the selected distribution. Each pane deletes its script on start; the last one removes the folder. A preparation error triggers cleanup. If Windows Terminal fails after the handoff, scripts may remain in `/tmp` until the distribution cleans it up.

Entered commands are Bash code executed with the WSL user's rights. The JSON file stores commands in plain text: secrets should stay in the environment or the project's usual tools.

This first version opens one tab per launch. It does not control panes that are already open, does not track server state and does not import batch shortcuts. Closing RunTerm leaves Windows Terminal running.

## License

MIT, see [LICENSE](LICENSE).
