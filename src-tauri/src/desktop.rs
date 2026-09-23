use runterm_core::{self as core, Config};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;

struct Storage(Mutex<()>);
fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config.json"))
}
#[tauri::command]
fn load_config(
    app: tauri::AppHandle,
    storage: tauri::State<Storage>,
) -> Result<Option<Config>, String> {
    let _guard = storage.0.lock().map_err(|e| e.to_string())?;
    core::load(&config_path(&app)?)
}
#[tauri::command]
fn save_config(
    app: tauri::AppHandle,
    storage: tauri::State<Storage>,
    config: Config,
) -> Result<(), String> {
    let _guard = storage.0.lock().map_err(|e| e.to_string())?;
    core::save(&config_path(&app)?, &config)
}
fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.iter().skip(1).step_by(2).take(20).any(|b| *b == 0)
    {
        let skip = usize::from(bytes.starts_with(&[0xff, 0xfe])) * 2;
        String::from_utf16_lossy(
            &bytes[skip..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}
fn execute(program: &str, args: &[String], input: Option<&[u8]>) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .creation_flags(0x08000000)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Impossible de lancer {program} : {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = stdout.read_to_end(&mut v);
        v
    });
    let err = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = stderr.read_to_end(&mut v);
        v
    });
    let writer = input.map(|bytes| {
        let bytes = bytes.to_vec();
        let mut stdin = child.stdin.take().unwrap();
        std::thread::spawn(move || stdin.write_all(&bytes))
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} ne répond pas après 30 secondes."));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let stdout = decode(&out.join().map_err(|_| "Erreur de lecture stdout.")?);
    let stderr = decode(&err.join().map_err(|_| "Erreur de lecture stderr.")?);
    if !status.success() {
        return Err(format!("{program} : {}{}", stderr.trim(), stdout.trim()));
    }
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| "Erreur d’écriture du script.")?
            .map_err(|e| e.to_string())?;
    }
    Ok(stdout)
}
/// Starts a program without waiting for it: unlike [`execute`], the child
/// keeps running, which a browser started from here does.
fn start(program: &str, args: &[String]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("Impossible de lancer {program} : {e}"))
}
/// String value of a registry entry: `/v <name>`, or `/ve` for the default one.
fn reg_query(key: &str, value: &[&str]) -> Option<String> {
    let mut args = vec!["query".to_string(), key.to_string()];
    args.extend(value.iter().map(|s| s.to_string()));
    execute("reg.exe", &args, None)
        .ok()
        .as_deref()
        .and_then(core::reg_string)
}
/// Executable of the browser handling `https`, read from the user's file
/// association. `None` when the association is missing or names no program.
fn default_browser() -> Option<String> {
    let prog_id = reg_query(
        r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        &["/v", "ProgId"],
    )?;
    // Interpolated into a registry path, so kept to the shape of a ProgId.
    if prog_id.is_empty()
        || !prog_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return None;
    }
    let command = reg_query(&format!(r"HKCR\{prog_id}\shell\open\command"), &["/ve"])?;
    core::browser_executable(&command)
}
/// Opens the project pages in the default browser. One browser process
/// receives them all, so they open as tabs of a single *new* window (see
/// [`core::browser_args`]) instead of the window the user is working in;
/// browsers forward them to their already running instance.
fn open_urls(urls: &[String]) -> Result<(), String> {
    if urls.is_empty() {
        return Ok(());
    }
    for url in urls {
        core::valid_url(url)?;
    }
    match default_browser() {
        Some(browser) => start(&browser, &core::browser_args(&browser, urls)),
        // No usable association: Windows opens each URL with its handler.
        None => urls.iter().try_for_each(|url| {
            start(
                "rundll32.exe",
                &["url.dll,FileProtocolHandler".into(), url.clone()],
            )
        }),
    }
}
/// `Code.exe`, read from the `vscode:` URL handler VS Code registers, or
/// found next to the `bin\code.cmd` of the `PATH`. `code.cmd` itself is not
/// run: it would go through `cmd.exe`.
fn vscode_executable() -> Option<String> {
    let registered = reg_query(r"HKCR\vscode\shell\open\command", &["/ve"])
        .as_deref()
        .and_then(core::browser_executable)
        .filter(|path| std::path::Path::new(path).is_file());
    registered.or_else(|| {
        let cmd = execute("where.exe", &["code.cmd".into()], None).ok()?;
        let code = PathBuf::from(cmd.lines().next()?.trim())
            .parent()?
            .parent()?
            .join("Code.exe");
        code.is_file().then(|| code.to_string_lossy().into_owned())
    })
}
fn wsl(distribution: &str, command: &[&str], input: Option<&[u8]>) -> Result<String, String> {
    let mut args = Vec::new();
    if !distribution.is_empty() {
        args.extend(["--distribution".into(), distribution.into()]);
    }
    args.push("--exec".into());
    args.extend(command.iter().map(|s| s.to_string()));
    execute("wsl.exe", &args, input)
}
fn distributions() -> Result<Vec<String>, String> {
    Ok(
        execute("wsl.exe", &["--list".into(), "--quiet".into()], None)?
            .lines()
            .map(|s| s.trim().trim_start_matches('\u{feff}').to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}
#[tauri::command]
async fn list_distributions() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(distributions)
        .await
        .map_err(|e| e.to_string())?
}
/// One line: `wsl.exe` receives it as a single command-line argument.
const LIST_DIRECTORIES: &str = r#"builtin cd -- "$1" || exit 1; shopt -s nullglob; for d in */; do d=${d%/}; [[ $d == *$'\n'* ]] || printf '%s\n' "$d"; done"#;
/// Visible subfolders of `path`, sorted; names containing a newline are skipped.
fn directories(distribution: &str, path: &str) -> Result<Vec<String>, String> {
    if !path.starts_with('/') || path.contains('\0') {
        return Err("La racine des workspaces doit être un chemin Linux absolu.".into());
    }
    let output = wsl(
        distribution,
        &[
            "bash",
            "--noprofile",
            "--norc",
            "-c",
            LIST_DIRECTORIES,
            "runterm",
            path,
        ],
        None,
    )
    .map_err(|_| format!("Dossier inaccessible : {path}"))?;
    let mut names: Vec<String> = output
        .lines()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort_by_key(|s| s.to_lowercase());
    Ok(names)
}
#[tauri::command]
async fn list_directories(distribution: String, path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || directories(&distribution, &path))
        .await
        .map_err(|e| e.to_string())?
}
/// A project ready to open: its scripts are written, nothing is started yet.
struct Prepared {
    layout: core::Layout,
    /// As configured: empty means the default distribution.
    distribution: String,
    profile: String,
    launches: BTreeMap<String, core::PaneLaunch>,
    /// Page to open in the default browser. Empty: none, or the project
    /// keeps its URL without opening it.
    url: String,
    /// `--folder-uri` opened in VS Code. Empty: none.
    vscode: String,
    /// Temporary folder holding the Bash scripts.
    folder: Option<String>,
}
impl Prepared {
    fn tab(&self) -> core::TabLaunch<'_> {
        core::TabLaunch {
            layout: &self.layout,
            distribution: &self.distribution,
            profile: &self.profile,
            launches: &self.launches,
        }
    }
    /// Removes the scripts of a project that will not be started.
    fn discard(&self) {
        if let Some(folder) = &self.folder {
            let _ = wsl(&self.distribution, &["rm", "-rf", "--", folder], None);
        }
    }
}
fn prepare(config: &Config, project_id: &str, available: &[String]) -> Result<Prepared, String> {
    let (project, template, panes) = config.resolve(project_id)?;
    if !project.distribution.is_empty() && !available.contains(&project.distribution) {
        return Err("La distribution WSL sélectionnée est introuvable.".into());
    }
    // Without an explicit profile: `wsl --list` puts the default distribution
    // first, and Windows Terminal names WSL profiles after their distribution.
    let distribution = if project.distribution.is_empty() {
        &available[0]
    } else {
        &project.distribution
    };
    let profile = if project.terminal_profile.is_empty() {
        distribution.clone()
    } else {
        project.terminal_profile.clone()
    };
    let vscode = project.vscode_uri(distribution);
    if !project.host.is_empty() {
        return prepare_ssh(project, template, &panes, profile, vscode);
    }
    for pane in &panes {
        wsl(
            &project.distribution,
            &[
                "bash",
                "--noprofile",
                "--norc",
                "-c",
                "test -d \"$1\" && test -x \"$1\"",
                "runterm",
                &pane.directory,
            ],
            None,
        )
        .map_err(|_| {
            format!(
                "Dossier inaccessible pour « {} » : {}",
                pane.name, pane.directory
            )
        })?;
    }
    // PowerShell 7 when installed, otherwise the built-in Windows PowerShell.
    let pwsh = panes.iter().any(|p| p.shell == core::Shell::Powershell)
        && execute("where.exe", &["pwsh.exe".into()], None).is_ok();
    let launches: BTreeMap<String, core::PaneLaunch> = panes
        .iter()
        .filter(|p| p.shell == core::Shell::Powershell)
        .map(|p| {
            // The Linux folder, reached from Windows through \\wsl.localhost.
            let directory = core::wsl_windows_path(distribution, &p.directory);
            let script = core::powershell_script(p, &directory);
            (p.id.clone(), core::PaneLaunch::Powershell { script, pwsh })
        })
        .collect();
    let mut prepared = Prepared {
        layout: template.layout.clone(),
        distribution: project.distribution.clone(),
        profile,
        url: project.url_to_open().to_owned(),
        vscode,
        launches,
        folder: None,
    };
    let bash: Vec<_> = panes
        .iter()
        .filter(|p| p.shell == core::Shell::Bash)
        .collect();
    if bash.is_empty() {
        return Ok(prepared);
    }
    let folder = wsl(
        &project.distribution,
        &[
            "bash",
            "--noprofile",
            "--norc",
            "-c",
            "umask 077; mktemp -d /tmp/runterm-XXXXXXXXXXXX",
        ],
        None,
    )?
    .trim()
    .to_string();
    if !folder.starts_with("/tmp/runterm-")
        || !folder
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/-".contains(&c))
    {
        return Err("WSL a retourné un chemin temporaire inattendu.".into());
    }
    prepared.folder = Some(folder.clone());
    let written = (|| {
        for pane in bash {
            let path = format!("{}/{}.sh", folder, uuid::Uuid::new_v4());
            let cleanup = format!(
                "rm -f -- {}\nrmdir -- {} 2>/dev/null || true\n",
                core::shell_quote(&path),
                core::shell_quote(&folder)
            );
            let script = format!("{}{}", cleanup, core::pane_script(pane));
            wsl(
                &project.distribution,
                &[
                    "bash",
                    "--noprofile",
                    "--norc",
                    "-c",
                    "umask 077; cat > \"$1\"",
                    "runterm",
                    &path,
                ],
                Some(script.as_bytes()),
            )?;
            prepared
                .launches
                .insert(pane.id.clone(), core::PaneLaunch::Bash(path));
        }
        // Fails early on a layout Windows Terminal cannot receive.
        core::tabs_args(&[prepared.tab()]).map(drop)
    })();
    match written {
        Ok(()) => Ok(prepared),
        Err(e) => {
            prepared.discard();
            Err(e)
        }
    }
}
/// Panes on an SSH host: their scripts travel in the `ssh` command line, so
/// nothing is written or checked beforehand; a missing folder is reported in
/// its pane.
fn prepare_ssh(
    project: &core::Project,
    template: &core::Template,
    panes: &[core::ResolvedPane],
    profile: String,
    vscode: String,
) -> Result<Prepared, String> {
    if let Some(pane) = panes.iter().find(|p| p.shell == core::Shell::Powershell) {
        return Err(format!(
            "Le panneau PowerShell « {} » ne peut pas s’ouvrir sur un hôte SSH.",
            pane.name
        ));
    }
    let prepared = Prepared {
        layout: template.layout.clone(),
        distribution: project.distribution.clone(),
        profile,
        url: project.url_to_open().to_owned(),
        vscode,
        launches: panes
            .iter()
            .map(|p| {
                let launch = core::PaneLaunch::Ssh {
                    host: project.host.clone(),
                    script: core::pane_script(p),
                };
                (p.id.clone(), launch)
            })
            .collect(),
        folder: None,
    };
    core::tabs_args(&[prepared.tab()])?;
    Ok(prepared)
}
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum LaunchMode {
    /// One window with a tab per project.
    Tabs,
    /// One window per project.
    Windows,
}
/// Prepares every project before opening anything: one failure starts none.
fn launch(config: Config, project_ids: Vec<String>, mode: LaunchMode) -> Result<(), String> {
    config.validate()?;
    if project_ids.is_empty() {
        return Err("Aucun projet à lancer.".into());
    }
    execute("where.exe", &["wt.exe".into()], None).map_err(|_| {
        "Windows Terminal est introuvable. Installez-le et activez son alias wt.exe.".to_string()
    })?;
    let available = distributions()?;
    if available.is_empty() {
        return Err("Aucune distribution WSL installée.".into());
    }
    let mut prepared: Vec<Prepared> = Vec::new();
    for id in &project_ids {
        match prepare(&config, id, &available) {
            Ok(p) => prepared.push(p),
            Err(e) => {
                prepared.iter().for_each(Prepared::discard);
                let name = config.projects.iter().find(|p| &p.id == id);
                return Err(match name {
                    Some(p) if project_ids.len() > 1 => format!("{} : {e}", p.name),
                    _ => e,
                });
            }
        }
    }
    // Before Windows Terminal, so its new window ends up in front. A browser
    // or a VS Code that cannot be started stops the launch, like any other
    // failure: the scripts are removed and nothing is opened.
    let mut urls: Vec<String> = Vec::new();
    let mut folders: Vec<String> = Vec::new();
    for project in &prepared {
        if !project.url.is_empty() && !urls.contains(&project.url) {
            urls.push(project.url.clone());
        }
        if !project.vscode.is_empty() && !folders.contains(&project.vscode) {
            folders.push(project.vscode.clone());
        }
    }
    let opened = (|| {
        // Looked up first, so a missing VS Code opens no browser either.
        let code = if folders.is_empty() {
            String::new()
        } else {
            vscode_executable().ok_or(
                "Visual Studio Code est introuvable. Installez-le, ou décochez « Ouvrir VS Code ».",
            )?
        };
        open_urls(&urls)?;
        // One window per folder; a running VS Code receives them and focuses
        // a window that already shows the folder.
        folders
            .iter()
            .try_for_each(|folder| start(&code, &["--folder-uri".into(), folder.clone()]))
    })();
    if let Err(e) = opened {
        prepared.iter().for_each(Prepared::discard);
        return Err(e);
    }
    // wt returns after dispatch; application processes are owned by Windows Terminal.
    match mode {
        LaunchMode::Tabs => {
            let tabs: Vec<_> = prepared.iter().map(Prepared::tab).collect();
            let started = core::tabs_args(&tabs).and_then(|args| execute("wt.exe", &args, None));
            if let Err(e) = started {
                prepared.iter().for_each(Prepared::discard);
                return Err(e);
            }
        }
        LaunchMode::Windows => {
            for (index, project) in prepared.iter().enumerate() {
                let started = core::tabs_args(&[project.tab()])
                    .and_then(|args| execute("wt.exe", &args, None));
                if let Err(e) = started {
                    prepared[index..].iter().for_each(Prepared::discard);
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
#[tauri::command]
async fn launch_projects(
    config: Config,
    project_ids: Vec<String>,
    mode: LaunchMode,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || launch(config, project_ids, mode))
        .await
        .map_err(|e| e.to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Storage(Mutex::new(())))
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            list_distributions,
            list_directories,
            launch_projects
        ])
        .run(tauri::generate_context!())
        .expect("Impossible de démarrer RunTerm");
}

#[cfg(test)]
mod tests {
    use super::*;
    use runterm_core::{Axis, Layout, Project, Template};

    #[test]
    fn decodes_windows_and_linux_command_output() {
        let utf16: Vec<u8> = "Ubuntu\r\nUbuntu-24.04\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode(&utf16), "Ubuntu\r\nUbuntu-24.04\r\n");
        assert_eq!(decode("Dossier prêt".as_bytes()), "Dossier prêt");
    }

    /// Opens a temporary Terminal window; intentionally excluded from unattended CI.
    #[test]
    #[ignore = "Requires Windows Terminal, WSL and an interactive Windows session"]
    fn windows_terminal_smoke() {
        let folder = wsl("", &["mktemp", "-d", "/tmp/runterm-smoke-XXXXXXXX"], None)
            .unwrap()
            .trim()
            .to_string();
        assert!(
            folder.starts_with("/tmp/runterm-smoke-")
                && folder
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"/-".contains(&c))
        );
        let make_pane = |id: &str| Layout::Pane {
            id: id.into(),
            name: format!("RunTerm test {id}"),
            directory: ".".into(),
            commands: vec![
                "export RUNTERM_SMOKE='value ; quoted'".into(),
                format!(
                    "printf '%s\\n' \"$PWD\" \"$RUNTERM_SMOKE\" > {}",
                    core::shell_quote(&format!("{folder}/{id}"))
                ),
                format!(
                    "for ((n=0;n<100;n++)); do [[ -f {} ]] && break; sleep .1; done; exit",
                    core::shell_quote(&format!("{folder}/release"))
                ),
            ],
            shell: core::Shell::Bash,
        };
        let split = |id: &str, axis, first, second| Layout::Split {
            id: id.into(),
            axis,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        };
        let layout = split(
            "root",
            Axis::Rows,
            split("top", Axis::Columns, make_pane("a"), make_pane("b")),
            split(
                "bottom",
                Axis::Columns,
                make_pane("c"),
                split("right", Axis::Columns, make_pane("d"), make_pane("e")),
            ),
        );
        let config = Config {
            schema_version: 1,
            workspace_root: String::new(),
            templates: vec![Template {
                id: "t".into(),
                name: "Smoke".into(),
                layout,
            }],
            projects: vec![Project {
                id: "p".into(),
                name: "Smoke".into(),
                root: folder.clone(),
                host: String::new(),
                distribution: String::new(),
                terminal_profile: String::new(),
                url: String::new(),
                url_disabled: false,
                vscode: false,
                vscode_folder: String::new(),
                template_id: "t".into(),
                overrides: BTreeMap::new(),
                launch_all: false,
            }],
        };
        let result = (|| -> Result<(), String> {
            launch(config, vec!["p".into()], LaunchMode::Windows)?;
            wsl("", &["bash", "--noprofile", "--norc", "-c", "for ((n=0;n<40;n++)); do if [[ -s $1/a && -s $1/b && -s $1/c && -s $1/d && -s $1/e ]]; then exit 0; fi; sleep .25; done; exit 1", "runterm", &folder], None)?;
            for id in ["a", "b", "c", "d", "e"] {
                let content = wsl("", &["cat", &format!("{folder}/{id}")], None)?;
                if content != format!("{folder}\nvalue ; quoted\n") {
                    return Err(format!("Panneau {id} : contenu inattendu : {content:?}"));
                }
            }
            Ok(())
        })();
        let _ = wsl("", &["touch", &format!("{folder}/release")], None);
        let _ = wsl(
            "",
            &[
                "bash",
                "--noprofile",
                "--norc",
                "-c",
                "sleep 1; rm -rf -- \"$1\"",
                "runterm",
                &folder,
            ],
            None,
        );
        assert!(result.is_ok(), "{:?}", result.err());
    }
}
