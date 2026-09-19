# Windows test plan

Run in the Windows Tauri app, with a test WSL distribution. The automated Linux tests cover the editor and the engine, but do not demonstrate Windows Terminal behavior.

1. In WSL, create two temporary projects, each containing `backend/` and `frontend/`.
2. Duplicate the developer template. Replace Codex and Claude with `printf 'assistant prêt\n'`, and Uvicorn with `python3 -m http.server 8765` (or a free port).
3. Bind both projects to the same template. Launch each one separately and check the five panes, their names, their directories (`pwd`) and the layout: two panes on top; shell on the bottom-left half; backend and frontend in the two remaining quarters.
4. Split another pane in the template, change the ratios, then relaunch: the layout must follow the changes. Remove that pane and check that its neighbor takes back the space.
5. Customize a command in the second project only. Then edit the template: the first project inherits, the second keeps its command. Use the reset button to restore inheritance.
6. In a pane, configure three actions: `export RUNTERM_CHECK='a ; b'`, `printf '%s\n' "$RUNTERM_CHECK"`, `pwd`. Check the order and the shared environment; the final prompt remains usable.
7. Configure `false` then `echo MUST_NOT_APPEAR`. Check that the sequence stops, the error message and the final prompt.
8. Add an action after the HTTP server. Stop the server with Ctrl+C: the prompt comes back and the next action does not start. Also check that a program exiting normally lets the sequence continue.
9. Test a root containing spaces and an apostrophe, as well as commands containing quotes, `$()`, semicolons and multiple lines. Only the entered commands must be interpreted as Bash.
10. Select a missing distribution (by editing a copy of the JSON) or a nonexistent directory. The app must show an error without opening a partial layout.
11. Save, close and reopen RunTerm: projects, templates and customizations are identical. Closing RunTerm after a launch leaves the terminals running.
12. On a backup copy, make the JSON invalid then relaunch: the GUI must refuse to open it without replacing the file.
13. Build the installer with `npm run tauri -- build`, install it and check startup and a WSL launch from the app shortcut.

The success message means the request was handed to Windows Terminal, not that every server started correctly. Program errors remain visible in their panes.

## Automated real launch test

In an interactive Windows session, after `npm run build`:

```powershell
cargo test -p runterm windows_terminal_smoke -- --ignored --nocapture
```

This test temporarily opens a five-pane window in the default WSL distribution. It checks the directories and the variables shared between actions using temporary files, then closes its shells. It does not launch Codex, Claude or the project servers. It does not visually check the pane geometry.
