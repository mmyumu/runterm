# Verification report — September 19, 2026

## Results

- TypeScript/Vite frontend build succeeds.
- 6 frontend tests pass: editing, saving and reloading a customized project; splitting/removing a pane; protection of a template in use; preservation of corrupted storage; inheritance and identifiers.
- 7 Rust tests pass on Linux: atomic save and refusal to overwrite corruption; project resolution; argument generation for nested layouts; validation; Bash sequences; Ctrl+C with a real pseudo-terminal; preservation of prompt hooks and single execution of actions.
- Clippy reports no warnings for the engine and the Windows code, tests included.
- Rust and frontend formatting checked.
- 4 engine tests also run with Windows Rust: all pass.
- 2 Windows integration tests pass: UTF-16/UTF-8 decoding and a real Windows Terminal/WSL launch.
- Real test: five panes run their actions in the right directory and share a variable between actions. The test shells then close automatically. No assistant or user project server is launched.
- UI checked in Chromium: creating, saving, selecting panes and adjusting a divider with the keyboard; no horizontal overflow at 1280 px.
- Windows executable launched: the WebView loads `http://tauri.localhost/`, shows the French UI and has access to the Tauri API. The embedded UI does not depend on Vite.
- Windows x64 NSIS installer built successfully. Deliverables and their checksums are in `artifacts/`.
- The npm audit run after upgrading Vitest reports no vulnerabilities.

## Validation limits

NSIS install/uninstall was not run. The app was checked directly from the compiled executable. The actual geometry of the five panes was not compared visually by automation; the generated arguments are tested, and the startup of the five shells is verified in Windows Terminal.

Windows build dependencies were used from a temporary folder, with the C++ compiler already installed. The native build used the same application code as the workspace. An extra Linux prompt test was added afterwards, without changing the application code.

The complementary manual test plan is in [windows-validation.md](windows-validation.md).

## Update: context menu

The WebView context menu is disabled globally. Checked in Chromium: `contextmenu` events on the UI background and on a button are cancelled. Frontend build, 6 frontend tests and formatting check pass.
