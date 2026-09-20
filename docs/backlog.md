# Backlog

Accepted ideas, roughly ordered by priority. Rejected ideas are listed at the
end so they are not proposed again.

## High

### Rearrange panes in the layout preview

Dragging a pane onto another swaps them; dragging it onto a split's edge moves
it into that branch. Today a pane can only be split or removed, so fixing a
layout means rebuilding it. Add layout presets (2x2, three columns, 1+2) and an
"equalize ratios" action next to them.

Touches: `LayoutPreview` in `src/App.tsx`, tree helpers in `src/model.ts`
(a `swapPanes`/`movePane` beside `updateNode`/`removePane`).

### Rework saving: autosave + persistent undo history

Today the whole `Config` is one state object: the **Enregistrer** button saves
everything, whatever page it is clicked from, and `dirty` compares the whole
config against `baseline`. That is not obvious from the UI, which shows the
button inside a template or project page.

Target: every edit is saved automatically, and **Ctrl+Z / Ctrl+Y** walk a
history that survives restarts, so a change made last week can still be undone.
The explicit **Annuler les modifications** and the `dirty` state disappear with
the save button.

Design:

- **Append-only log on disk**, next to the config:
  `%APPDATA%\dev.runterm.desktop\history\<n>.json`, one full config snapshot
  per version (a config is a few KB; snapshots keep the format trivial and the
  restore exact — no diff format to maintain). A `head` file holds the version
  the app currently shows, so undo/redo is moving that pointer.
- **Coalescing**, so typing a name does not create twenty versions: a new
  version is written after a short idle delay, and consecutive edits to the same
  field fold into the current one. Identical content never creates a version.
- **Redo tail**: editing after an undo truncates the versions above the pointer,
  as in any editor.
- **Pruning** keeps it honest: drop the oldest versions past a generous cap
  (order of a few thousand versions / a few tens of MB), never the recent ones.
- This log replaces the `config.json.bak` idea: it already is the safety net,
  and a `config.json` that fails to parse can offer a restore from the last
  version that does.
- A **history panel** (list of versions with timestamps, "revenir à cette
  version") makes a stack that spans sessions discoverable — otherwise Ctrl+Z
  right after opening the app silently undoes something from yesterday.

Touches: `src/App.tsx` (state, autosave, keybindings, history panel),
`crates/core/src/lib.rs` (`save`, history read/write/prune),
`src-tauri/src/desktop.rs` (commands for the history directory).

## Medium

### Create a project from a folder

Point at a folder, get a prefilled template instead of composing one from
scratch. Detection from marker files (`package.json` scripts, `pyproject.toml`,
`Cargo.toml`, `docker-compose.yml`, `Makefile`, `.git`) covers the common cases
but not the long tail. Open question: keep it to a small, honest set of rules
that proposes a *draft* the user edits, rather than aiming for correctness.

Touches: a `inspect_directory` Tauri command in `src-tauri/src/desktop.rs`, a
wizard in `src/App.tsx`.

### Recents and sidebar search

`lastLaunchedAt` per project (schema change in both models), a "recent" sort and
a filter field once the list gets long. Also the data a tray menu would need.

### Tab color and title per project

`wt` accepts `--tabColor` and `--title`. A color chip per project, visible both
in the sidebar and on the Windows Terminal tab; worth most in "Onglets" mode
with several projects.

Touches: `Project` in both models, `tab_args` in `crates/core/src/lib.rs`.

## Low

### Project diagnostics

A **Diagnostiquer** button running non-blocking checks and reporting a
green/orange list: distribution present, root exists, each pane directory
exists, each first command resolvable (`command -v`), SSH host reachable
(`ssh -o BatchMode=yes -o ConnectTimeout=5 <host> true`), Windows Terminal
profile known. Only worth shipping if the checks are reliable enough not to cry
wolf — a false orange is worse than no check.

### Wait for a dependency before running a pane's actions

A `waitFor` field per pane (TCP port or URL) generating a bounded
`until … ; do sleep 0.3; done` at the top of the pane script, so a front end
does not open while its API is still starting. Pure `pane_script` work, unit
testable. No current need.

### `RunTerm.exe --launch <project>`

CLI mode on the Tauri binary (single-instance plugin, argv parsing): launch a
project and exit without showing the window. Enables taskbar shortcuts, `.lnk`
files, Windows hotkeys, scheduled tasks — and is the groundwork for a tray icon
listing the projects.

### Launch preview

A `preview_launch` command returning the generated `wt.exe` arguments and the
pane scripts, shown in a collapsed panel with a "copy" button. Must stay
discreet: hidden by default, no extra step in the launch flow. Main value is
diagnosing a pane that does not start, without a Windows session.

## Rejected

- **Project variables / command interpolation** (`${PORT}` in commands, an `env`
  map per project): factoring out what similar projects share is not a need.
- **Config import/export**: single user, and the two machines hold different
  projects.
- **Launch into the existing Terminal window** (`wt -w 0`): rare enough that it
  is not worth a choice at every launch.
