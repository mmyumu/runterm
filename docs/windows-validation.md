# Windows test plan

Run in the Windows Tauri app, with a test WSL distribution. The automated Linux tests cover the editor and the engine, but do not demonstrate Windows Terminal behavior.

1. In WSL, create two temporary projects, each containing `backend/` and `frontend/`.
2. Duplicate the developer template. Replace Codex and Claude with `printf 'assistant prêt\n'`, and Uvicorn with `python3 -m http.server 8765` (or a free port).
3. Bind both projects to the same template. Launch each one separately and check the five panes, their names, their directories (`pwd`) and the layout: two panes on top; shell on the bottom-left half; backend and frontend in the two remaining quarters.
4. Split another pane in the template, change the ratios, then relaunch: the layout must follow the changes. Remove that pane and check that its neighbor takes back the space.
5. Customize a command in the second project only. Then edit the template: the first project inherits, the second keeps its command. Use the reset button to restore inheritance.
6. In a pane, configure three actions: `export RUNTERM_CHECK='a ; b'`, `printf '%s\n' "$RUNTERM_CHECK"`, `pwd`. Check the order and the shared environment; the final prompt remains usable.
7. Configure `false` then `echo MUST_NOT_APPEAR`. Check that the sequence stops, the error message and the final prompt.
8. Add an action after the HTTP server. Stop the server with Ctrl+C: the prompt comes back and the next action does not start. Also check that a program exiting normally lets the sequence continue. Then press the up arrow: the server command line comes back, ready to be replayed.
9. Test a root containing spaces and an apostrophe, as well as commands containing quotes, `$()`, semicolons and multiple lines. Only the entered commands must be interpreted as Bash.
10. Select a missing distribution (by editing a copy of the JSON) or a nonexistent directory. The app must show an error without opening a partial layout.
11. Save, close and reopen RunTerm: projects, templates and customizations are identical. Closing RunTerm after a launch leaves the terminals running.
12. On a backup copy, make the JSON invalid then relaunch: the GUI must refuse to open it without replacing the file.
13. In **Paramètres**, set the workspaces root to the folder holding the two test projects. Create a project: its root is prefilled, the root field suggests both subfolders (hidden folders excluded), and picking one names the project after it. A manually typed path is still accepted. A nonexistent workspaces root yields no suggestions and no error.
14. Switch a pane of the template to **PowerShell (Windows)** with the actions `$v = 'a ; b'`, `Write-Host $v`, `cmd.exe /c exit 3`, `Write-Host MUST_NOT_APPEAR`. Launch: the pane opens with the PowerShell profile, `Get-Location` shows the project folder under `\\wsl.localhost\`, `a ; b` is printed, the sequence stops at action 3 and the prompt stays usable. Check with and without PowerShell 7 installed, and a template made only of PowerShell panes (no `/tmp/runterm-*` folder is created).
15. Create a second project on another template (and, if available, another distribution). Toggle the list icon on both, save, close and reopen RunTerm: both stay toggled. **Tout lancer → Onglets** opens one window with one tab per project, the first tab focused; **Fenêtres** opens one window per project. Point one project at a nonexistent directory: the error names that project, nothing opens and no `/tmp/runterm-*` folder is left behind.
16. On a project, set **Hôte SSH** to a reachable host (a VPS or VM with key authentication) and a root that exists there, with actions such as `export V='a ; "b"'`, `echo "$V $PWD"`, `false`, `echo MUST_NOT_APPEAR`. Launch: every pane connects without writing anything under `/tmp` locally or on the host, prints `a ; "b"` with the pane's remote directory, stops at action 3 and keeps a remote prompt. Also check an alias from the WSL `~/.ssh/config`, a nonexistent remote directory (the pane shows « RunTerm : dossier inaccessible. »), an unreachable host (the pane shows the `ssh` error) and a template with a PowerShell pane (launch refused, nothing opens).
17. On a new project, **Ouvrir au lancement** starts cleared; check it, then set **URL à ouvrir** to `http://localhost:8765` on the first project and `https://example.com/docs` on the second, then launch the first alone: the page opens in the default browser and the Windows Terminal window is in front. **Tout lancer** with both projects: one browser window with one tab per project, and with Edge or Chrome that window is a new one, leaving the windows already open untouched. Repeat with the browser closed beforehand, and once with a project whose URL is duplicated (a single tab). Check with Chrome, Edge and Firefox as the default browser, and with an empty URL (no browser opens). Clear **Ouvrir au lancement** on one project, save, close and reopen RunTerm: the address is still there, unchecked, and launching that project opens no browser while the other project's URL still opens.
18. Build the installer with `npm run tauri -- build`, install it and check startup and a WSL launch from the app shortcut.

The success message means the request was handed to Windows Terminal, not that every server started correctly. Program errors remain visible in their panes.

## Automated real launch test

In an interactive Windows session, after `npm run build`:

```powershell
cargo test -p runterm windows_terminal_smoke -- --ignored --nocapture
```

This test temporarily opens a five-pane window in the default WSL distribution. It checks the directories and the variables shared between actions using temporary files, then closes its shells. It does not launch Codex, Claude or the project servers. It does not visually check the pane geometry.
