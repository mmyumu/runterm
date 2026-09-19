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
fn launch(config: Config, project_id: String) -> Result<(), String> {
    let (project, template, panes) = config.resolve(&project_id)?;
    execute("where.exe", &["wt.exe".into()], None).map_err(|_| {
        "Windows Terminal est introuvable. Installez-le et activez son alias wt.exe.".to_string()
    })?;
    let available = distributions()?;
    if available.is_empty() {
        return Err("Aucune distribution WSL installée.".into());
    }
    if !project.distribution.is_empty() && !available.contains(&project.distribution) {
        return Err("La distribution WSL sélectionnée est introuvable.".into());
    }
    // Without an explicit profile: `wsl --list` puts the default distribution
    // first, and Windows Terminal names WSL profiles after their distribution.
    let profile = if !project.terminal_profile.is_empty() {
        project.terminal_profile.clone()
    } else if project.distribution.is_empty() {
        available[0].clone()
    } else {
        project.distribution.clone()
    };
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
    let prepared = (|| {
        let mut scripts = BTreeMap::new();
        for pane in &panes {
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
            scripts.insert(pane.id.clone(), path);
        }
        core::terminal_args(&template.layout, &project.distribution, &profile, &scripts)
    })();
    let args = match prepared {
        Ok(args) => args,
        Err(e) => {
            let _ = wsl(&project.distribution, &["rm", "-rf", "--", &folder], None);
            return Err(e);
        }
    };
    // wt returns after dispatch; application processes are owned by Windows Terminal.
    execute("wt.exe", &args, None)?;
    Ok(())
}
#[tauri::command]
async fn launch_project(config: Config, project_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || launch(config, project_id))
        .await
        .map_err(|e| e.to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .manage(Storage(Mutex::new(())))
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            list_distributions,
            launch_project
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
            templates: vec![Template {
                id: "t".into(),
                name: "Smoke".into(),
                layout,
            }],
            projects: vec![Project {
                id: "p".into(),
                name: "Smoke".into(),
                root: folder.clone(),
                distribution: String::new(),
                terminal_profile: String::new(),
                template_id: "t".into(),
                overrides: BTreeMap::new(),
            }],
        };
        let result = (|| -> Result<(), String> {
            launch(config, "p".into())?;
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
