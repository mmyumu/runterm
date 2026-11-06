use super::*;
fn pane(id: &str) -> Layout {
    Layout::Pane {
        id: id.into(),
        name: id.into(),
        directory: ".".into(),
        commands: vec!["echo ok".into()],
        shell: Shell::Bash,
    }
}
fn config() -> Config {
    Config {
        schema_version: 1,
        workspace_root: String::new(),
        templates: vec![Template {
            id: "t".into(),
            name: "Dev".into(),
            layout: pane("a"),
        }],
        projects: vec![Project {
            id: "p".into(),
            name: "Projet".into(),
            root: "/home/me/project".into(),
            host: String::new(),
            distribution: "Ubuntu".into(),
            terminal_profile: String::new(),
            url: String::new(),
            url_disabled: false,
            vscode: false,
            vscode_folder: String::new(),
            template_id: "t".into(),
            overrides: BTreeMap::new(),
            launch_all: false,
        }],
        launch_groups: Vec::new(),
    }
}
#[test]
fn overrides_and_distinct_roots() {
    let mut c = config();
    c.projects[0].overrides.insert(
        "a".into(),
        Override {
            directory: Some("backend".into()),
            commands: Some(vec![]),
        },
    );
    let mut other = c.projects[0].clone();
    other.id = "q".into();
    other.root = "/other".into();
    c.projects.push(other);
    let (_, _, p) = c.resolve("p").unwrap();
    assert_eq!(p[0].directory, "/home/me/project/backend");
    assert!(p[0].commands.is_empty());
    assert_eq!(c.resolve("q").unwrap().2[0].directory, "/other/backend");
    c.projects[0].overrides.clear();
    assert_eq!(c.resolve("p").unwrap().2[0].commands, vec!["echo ok"]);
}
#[test]
fn persistence_refuses_corruption() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("config.json");
    let c = config();
    save(&p, &c).unwrap();
    assert_eq!(load(&p).unwrap(), Some(c.clone()));
    save(&p, &c).unwrap();
    fs::write(&p, "broken").unwrap();
    assert!(save(&p, &c).is_err());
    assert_eq!(fs::read_to_string(p).unwrap(), "broken");
}
#[test]
fn nested_layout_navigation() {
    let l = Layout::Split {
        id: "root".into(),
        axis: Axis::Rows,
        ratio: 0.4,
        first: Box::new(Layout::Split {
            id: "nested".into(),
            axis: Axis::Columns,
            ratio: 0.7,
            first: Box::new(pane("a")),
            second: Box::new(pane("b")),
        }),
        second: Box::new(pane("c")),
    };
    let scripts = ["a", "b", "c"]
        .map(|id| {
            (
                id.into(),
                PaneLaunch::Bash(format!("/tmp/runterm-test/{id}.sh")),
            )
        })
        .into();
    let args = terminal_args(&l, "Ubuntu", "Ubuntu", &scripts).unwrap();
    assert_eq!(args.iter().filter(|a| *a == "split-pane").count(), 2);
    assert!(args.windows(3).any(|a| a == ["-H", "--size", "0.600000"]));
    assert!(args.windows(3).any(|a| a == ["-V", "--size", "0.300000"]));
    // Every split targets its pane by id rather than trusting the current focus.
    for i in (0..args.len()).filter(|&i| args[i] == "split-pane") {
        assert_eq!(args[i - 4..i - 2], ["focus-pane", "--target"]);
    }
    // Programs (e.g. Claude Code) must be able to update the tab title.
    assert!(args.iter().all(|a| a != "--suppressApplicationTitle"));
    assert_eq!(
        args.windows(2)
            .filter(|a| a == &["--profile", "Ubuntu"])
            .count(),
        3
    );
    assert!(terminal_args(&pane("a"), "", "", &scripts)
        .unwrap()
        .iter()
        .all(|a| a != "split-pane"));
}
/// Replays wt.exe arguments the way Windows Terminal builds its pane tree and
/// returns it in the same textual form as `expected_tree`.
fn simulate_windows_terminal(args: &[String]) -> String {
    enum Node {
        Leaf(String),
        Split(String, String, Box<Node>, Box<Node>),
    }
    fn split(n: &mut Node, target: &str, dir: &str, size: &str, title: &str) {
        match n {
            Node::Leaf(t) if t == target => {
                let old = std::mem::take(t);
                *n = Node::Split(
                    dir.into(),
                    size.into(),
                    Box::new(Node::Leaf(old)),
                    Box::new(Node::Leaf(title.into())),
                );
            }
            Node::Leaf(_) => {}
            Node::Split(_, _, a, b) => {
                split(a, target, dir, size, title);
                split(b, target, dir, size, title);
            }
        }
    }
    fn render(n: &Node) -> String {
        match n {
            Node::Leaf(t) => t.clone(),
            Node::Split(d, s, a, b) => format!("{d} {s}({},{})", render(a), render(b)),
        }
    }
    let title = |c: &[String]| c[c.iter().position(|a| a == "--title").unwrap() + 1].clone();
    // Pane ids, numbered per tab in creation order.
    let (mut root, mut ids, mut focused) = (None::<Node>, Vec::new(), String::new());
    for command in args.split(|a| a == ";") {
        match command
            .iter()
            .position(|a| a == "new-tab" || a == "split-pane" || a == "focus-pane")
        {
            Some(i) if command[i] == "new-tab" => {
                focused = title(command);
                root = Some(Node::Leaf(focused.clone()));
                ids = vec![focused.clone()];
            }
            Some(i) if command[i] == "split-pane" => {
                let new = title(command);
                split(
                    root.as_mut().unwrap(),
                    &focused,
                    &command[i + 1],
                    &command[i + 3],
                    &new,
                );
                ids.push(new.clone());
                focused = new;
            }
            Some(i) => {
                assert_eq!(command[i + 1], "--target");
                focused = ids[command[i + 2].parse::<usize>().unwrap()].clone();
            }
            None => panic!("commande inattendue {command:?}"),
        }
    }
    render(root.as_ref().unwrap()) + " focus=" + &focused
}
fn expected_tree(l: &Layout) -> String {
    match l {
        Layout::Pane { name, .. } => name.clone(),
        Layout::Split {
            axis,
            ratio,
            first,
            second,
            ..
        } => format!(
            "{} {:.6}({},{})",
            if *axis == Axis::Columns { "-V" } else { "-H" },
            1.0 - ratio,
            expected_tree(first),
            expected_tree(second)
        ),
    }
}
fn split(axis: Axis, first: Layout, second: Layout) -> Layout {
    Layout::Split {
        id: format!("s-{}", uuid_like(&first, &second)),
        axis,
        ratio: 0.5,
        first: Box::new(first),
        second: Box::new(second),
    }
}
fn uuid_like(a: &Layout, b: &Layout) -> String {
    format!("{}{}", expected_tree(a), expected_tree(b))
        .replace(|c: char| !c.is_ascii_alphanumeric(), "")
}
#[test]
fn terminal_args_reproduce_layout_tree() {
    use Axis::{Columns as C, Rows as R};
    let layouts = [
        pane("a"),
        split(
            C,
            split(R, pane("a"), pane("b")),
            split(R, pane("c"), pane("d")),
        ),
        split(
            R,
            split(C, pane("a"), split(R, pane("b"), pane("c"))),
            split(C, pane("d"), pane("e")),
        ),
        split(
            C,
            split(
                R,
                split(C, pane("a"), pane("b")),
                split(C, pane("c"), pane("d")),
            ),
            split(
                R,
                split(C, pane("e"), pane("f")),
                split(C, pane("g"), pane("h")),
            ),
        ),
    ];
    for l in layouts {
        let mut ids = Vec::new();
        fn collect(l: &Layout, ids: &mut Vec<String>) {
            match l {
                Layout::Pane { id, .. } => ids.push(id.clone()),
                Layout::Split { first, second, .. } => {
                    collect(first, ids);
                    collect(second, ids);
                }
            }
        }
        collect(&l, &mut ids);
        let scripts = ids
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    PaneLaunch::Bash(format!("/tmp/runterm-test/{id}.sh")),
                )
            })
            .collect();
        let args = terminal_args(&l, "Ubuntu", "Ubuntu", &scripts).unwrap();
        assert_eq!(
            simulate_windows_terminal(&args),
            format!("{} focus=a", expected_tree(&l))
        );
    }
}
#[test]
fn terminal_profile_is_optional_and_validated() {
    let mut c = config();
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains("terminalProfile"));
    assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), c);
    c.projects[0].terminal_profile = "Ubuntu 24.04.1 LTS".into();
    assert!(c.validate().is_ok());
    for bad in ["a;b", "a\nb", "--help"] {
        c.projects[0].terminal_profile = bad.into();
        assert!(c.validate().is_err(), "{bad}");
    }
}
#[test]
fn url_is_optional_and_validated() {
    let mut c = config();
    let json = serde_json::to_value(&c).unwrap();
    assert!(json["projects"][0].get("url").is_none());
    for good in [
        "http://localhost:5173",
        "https://app.example.com/dashboard?tab=1#top",
        "http://127.0.0.1:8000/docs",
    ] {
        c.projects[0].url = good.into();
        assert!(c.validate().is_ok(), "{good}");
    }
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["projects"][0]["url"], "http://127.0.0.1:8000/docs");
    assert_eq!(serde_json::from_value::<Config>(json).unwrap(), c);
    for bad in [
        "localhost:5173",
        "file:///C:/page.html",
        "http://",
        "http://a b",
        "http://a\nb",
        "--no-sandbox",
        "http://a\u{0}b",
    ] {
        c.projects[0].url = bad.into();
        assert!(c.validate().is_err(), "{bad:?}");
    }
}
#[test]
fn a_disabled_url_is_kept_but_not_opened() {
    let mut c = config();
    c.projects[0].url = "http://localhost:5173".into();
    assert_eq!(c.projects[0].url_to_open(), "http://localhost:5173");
    let json = serde_json::to_value(&c).unwrap();
    assert!(json["projects"][0].get("urlDisabled").is_none());
    c.projects[0].url_disabled = true;
    // The address stays in the file, and is still held to the same rules.
    assert!(c.validate().is_ok());
    assert_eq!(c.projects[0].url, "http://localhost:5173");
    assert_eq!(c.projects[0].url_to_open(), "");
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["projects"][0]["urlDisabled"], true);
    assert_eq!(serde_json::from_value::<Config>(json).unwrap(), c);
}
#[test]
fn vscode_opens_the_root_or_another_folder_remotely() {
    let mut c = config();
    assert_eq!(c.projects[0].vscode_uri("Ubuntu"), "");
    let json = serde_json::to_value(&c).unwrap();
    assert!(json["projects"][0].get("vscode").is_none());
    assert!(json["projects"][0].get("vscodeFolder").is_none());
    c.projects[0].vscode = true;
    assert_eq!(
        c.projects[0].vscode_uri("Ubuntu"),
        "vscode-remote://wsl+Ubuntu/home/me/project"
    );
    for (folder, uri) in [
        (".", "vscode-remote://wsl+Ubuntu/home/me/project"),
        (
            "frontend/",
            "vscode-remote://wsl+Ubuntu/home/me/project/frontend",
        ),
        (
            "/srv/other app",
            "vscode-remote://wsl+Ubuntu/srv/other%20app",
        ),
        ("/", "vscode-remote://wsl+Ubuntu/"),
        (
            "été#1",
            "vscode-remote://wsl+Ubuntu/home/me/project/%C3%A9t%C3%A9%231",
        ),
    ] {
        c.projects[0].vscode_folder = folder.into();
        assert!(c.validate().is_ok(), "{folder}");
        assert_eq!(c.projects[0].vscode_uri("Ubuntu"), uri, "{folder}");
    }
    c.projects[0].vscode_folder = "back\nend".into();
    assert!(c.validate().is_err());
    // Over SSH, the host names the remote; `@` and `:` stay out of the authority syntax.
    c.projects[0].vscode_folder = String::new();
    c.projects[0].host = "me@dev:2222".into();
    assert_eq!(
        c.projects[0].vscode_uri("Ubuntu"),
        "vscode-remote://ssh-remote+me%40dev%3A2222/home/me/project"
    );
    c.projects[0].vscode_folder = "api".into();
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["projects"][0]["vscode"], true);
    assert_eq!(json["projects"][0]["vscodeFolder"], "api");
    assert_eq!(serde_json::from_value::<Config>(json).unwrap(), c);
}
#[test]
fn default_browser_is_read_from_its_registry_command() {
    let chrome = "    (Default)    REG_SZ    \"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" --single-argument %1";
    assert_eq!(
        reg_string(&format!(
            "\r\nHKEY_CLASSES_ROOT\\ChromeHTML\\shell\\open\\command\r\n{chrome}\r\n"
        ))
        .as_deref()
        .and_then(browser_executable),
        Some("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".into())
    );
    // A localized entry name, an expandable value and an unquoted path with spaces.
    let firefox = "    (Par défaut)    REG_EXPAND_SZ    C:\\Program Files\\Mozilla Firefox\\firefox.exe -osint -url \"%1\"";
    assert_eq!(
        reg_string(firefox).as_deref().and_then(browser_executable),
        Some("C:\\Program Files\\Mozilla Firefox\\firefox.exe".into())
    );
    assert_eq!(
        reg_string("    ProgId    REG_SZ    ChromeHTML").as_deref(),
        Some("ChromeHTML")
    );
    assert_eq!(reg_string("ERROR: cannot find the key"), None);
    // A Store application, or anything else RunTerm cannot pass URLs to.
    for bad in ["", "\"%1\"", "AppX4hxtad77fbk3jkkeerkrm0ze94wjf3s9"] {
        assert_eq!(browser_executable(bad), None, "{bad:?}");
    }
}
#[test]
fn chromium_browsers_get_a_new_window_for_every_url() {
    let urls = [
        "http://localhost:8765".to_owned(),
        "https://example.com".to_owned(),
    ];
    // One flag for the whole command line: the URLs are tabs of that one window.
    assert_eq!(
        browser_args(
            "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\MSEDGE.EXE",
            &urls
        ),
        [
            "--new-window",
            "http://localhost:8765",
            "https://example.com"
        ]
    );
    assert_eq!(
        browser_args(
            "C:/Program Files/Google/Chrome/Application/chrome.exe",
            &urls
        )[0],
        "--new-window"
    );
    // Firefox has no flag that opens several URLs in one new window.
    assert_eq!(
        browser_args("C:\\Program Files\\Mozilla Firefox\\firefox.exe", &urls),
        urls
    );
    assert_eq!(browser_args("chrome.exe", &[]), ["--new-window"]);
}
#[test]
fn workspace_root_is_optional_and_validated() {
    let mut c = config();
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains("workspaceRoot"));
    assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), c);
    c.workspace_root = "/home/me/workspaces".into();
    assert!(c.validate().is_ok());
    assert!(serde_json::to_string(&c)
        .unwrap()
        .contains(r#""workspaceRoot":"/home/me/workspaces""#));
    for bad in ["workspaces", "/home/\0"] {
        c.workspace_root = bad.into();
        assert!(c.validate().is_err(), "{bad}");
    }
}
#[test]
fn shell_defaults_to_bash_and_is_omitted() {
    let mut c = config();
    let json = serde_json::to_string(&c).unwrap();
    assert!(!json.contains("shell"));
    assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), c);
    let Layout::Pane { shell, .. } = &mut c.templates[0].layout else {
        unreachable!()
    };
    *shell = Shell::Powershell;
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains(r#""shell":"powershell""#));
    assert_eq!(serde_json::from_str::<Config>(&json).unwrap(), c);
    assert_eq!(c.resolve("p").unwrap().2[0].shell, Shell::Powershell);
}
#[test]
fn powershell_quoting_and_paths() {
    assert_eq!(powershell_quote("it's"), "'it''s'");
    assert_eq!(powershell_quote("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
    assert_eq!(powershell_quote("$x `n"), "'$x `n'");
    assert_eq!(
        wsl_windows_path("Ubuntu", "/home/me/my project/."),
        r"\\wsl.localhost\Ubuntu\home\me\my project\."
    );
    // `[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes(...))`
    assert_eq!(encode_powershell("a"), "YQA=");
    assert_eq!(encode_powershell("ab"), "YQBiAA==");
    assert_eq!(encode_powershell("abc"), "YQBiAGMA");
    assert_eq!(encode_powershell("é;"), "6QA7AA==");
}
#[test]
fn powershell_panes_use_their_own_program_and_profile() {
    let l = split(Axis::Columns, pane("a"), pane("b"));
    let launches = BTreeMap::from([
        (
            "a".into(),
            PaneLaunch::Bash("/tmp/runterm-test/a.sh".into()),
        ),
        (
            "b".into(),
            PaneLaunch::Powershell {
                script: "Write-Host 'a ; b'".into(),
                pwsh: false,
            },
        ),
    ]);
    let args = terminal_args(&l, "Ubuntu", "Ubuntu", &launches).unwrap();
    let encoded = encode_powershell("Write-Host 'a ; b'");
    assert!(args.windows(10).any(|a| a
        == [
            "--profile",
            "Windows PowerShell",
            "--title",
            "b",
            "--",
            "powershell.exe",
            "-NoLogo",
            "-NoExit",
            "-EncodedCommand",
            encoded.as_str(),
        ]));
    // Windows Terminal would split the command on any `;` argument content.
    assert!(args.iter().all(|a| a == ";" || !a.contains(';')));
    assert_eq!(args.iter().filter(|a| *a == "wsl.exe").count(), 1);
    let pwsh = BTreeMap::from([(
        "a".into(),
        PaneLaunch::Powershell {
            script: String::new(),
            pwsh: true,
        },
    )]);
    let args = terminal_args(&pane("a"), "", "Ubuntu", &pwsh).unwrap();
    assert!(args.windows(2).any(|a| a == ["--profile", "PowerShell"]));
    assert!(args.iter().any(|a| a == "pwsh.exe"));
    let long = BTreeMap::from([(
        "a".into(),
        PaneLaunch::Powershell {
            script: "x".repeat(20_000),
            pwsh: false,
        },
    )]);
    assert!(terminal_args(&pane("a"), "", "", &long).is_err());
}
#[cfg(windows)]
#[test]
fn powershell_sequence_preserves_scope_and_stops_on_failure() {
    use std::process::Command;
    let d = tempfile::tempdir().unwrap();
    let output_file = d.path().join("result ; 'quoted'.txt");
    let p = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: String::new(),
        commands: vec![
            "$value = 'hello ; world'\nfunction Get-Kept { 'kept' }".into(),
            format!(
                "Set-Content -LiteralPath {} -Value \"$value/$(Get-Kept)/$(Split-Path -Leaf $PWD)\"",
                powershell_quote(output_file.to_str().unwrap())
            ),
            "cmd.exe /c exit 3".into(),
            "Set-Content should-not-exist bad".into(),
        ],
        shell: Shell::Powershell,
    };
    let script = powershell_script(&p, d.path().to_str().unwrap(), &[]);
    let out = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encode_powershell(&script),
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("action 3 interrompue (code 3)"));
    assert_eq!(
        fs::read_to_string(output_file).unwrap().trim_end(),
        format!(
            "hello ; world/kept/{}",
            d.path().file_name().unwrap().to_str().unwrap()
        )
    );
    assert!(!d.path().join("should-not-exist").exists());
}
#[test]
fn validation_rejects_missing_template_and_invalid_ratios() {
    let mut c = config();
    c.projects[0].template_id = "missing".into();
    assert!(c.validate().is_err());
    c = config();
    c.projects[0].overrides.insert(
        "a".into(),
        Override {
            directory: Some("../escape".into()),
            commands: None,
        },
    );
    assert!(c.validate().is_err());
}
#[cfg(unix)]
#[test]
fn bash_sequence_preserves_environment_and_stops_on_failure() {
    use std::process::{Command, Stdio};
    let d = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let output_file = d.path().join("result ; 'quoted'.txt");
    let p = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: d.path().to_str().unwrap().into(),
        commands: vec![
            "export RUNTERM_TEST='hello ; world'\nlocal_value=kept".into(),
            format!(
                "printf '%s\\n' \"$RUNTERM_TEST/$local_value\" > {}",
                shell_quote(output_file.to_str().unwrap())
            ),
            "false".into(),
            "echo bad > should-not-exist".into(),
        ],
        shell: Shell::Bash,
    };
    let rc = d.path().join("rc");
    fs::write(&rc, pane_script(&p, &[])).unwrap();
    let mut child = Command::new("bash")
        .args(["--noprofile", "--rcfile", rc.to_str().unwrap(), "-i"])
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf 'PROMPT-STILL-ALIVE\\n'\nexit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("PROMPT-STILL-ALIVE"));
    assert_eq!(
        fs::read_to_string(output_file).unwrap(),
        "hello ; world/kept\n"
    );
    assert!(!d.path().join("should-not-exist").exists());
}

#[cfg(unix)]
#[test]
fn actions_are_recallable_from_the_shell_history() {
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join(".bashrc"), "HISTCONTROL=ignoreboth\n").unwrap();
    let pane = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: dir.path().to_str().unwrap().into(),
        commands: vec![
            "export RUNTERM_TEST=1".into(),
            "  python3 server.py --db 'it is fine.sqlite3'".into(),
        ],
        shell: Shell::Bash,
    };
    let rc = dir.path().join("rc");
    fs::write(&rc, pane_script(&pane, &[])).unwrap();
    let dump = dir.path().join("history");
    let mut child = Command::new("bash")
        .args(["--noprofile", "--rcfile", rc.to_str().unwrap(), "-i"])
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("history -w {}\nexit\n", shell_quote(dump.to_str().unwrap())).as_bytes())
        .unwrap();
    child.wait_with_output().unwrap();
    let history: Vec<String> = fs::read_to_string(&dump)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    // The second action failed (no server.py), so only the recall matters here:
    // both actions are there, in order, and the last one is one up arrow away.
    let first = history
        .iter()
        .position(|l| l == "export RUNTERM_TEST=1")
        .unwrap_or_else(|| panic!("{history:?}"));
    assert_eq!(
        history[first + 1],
        "python3 server.py --db 'it is fine.sqlite3'",
        "{history:?}"
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_interrupts_sequence_and_keeps_interactive_prompt() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let rc = dir.path().join("rc");
    let pane = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: dir.path().to_str().unwrap().into(),
        commands: vec![
            // Printed by the child so the marker means it already owns the terminal:
            // an earlier Ctrl+C would reach Bash, whose trap waits for `sleep` to end.
            "sh -c 'echo __READY__; exec sleep 30'".into(),
            "touch should-not-exist".into(),
        ],
        shell: Shell::Bash,
    };
    fs::write(&rc, pane_script(&pane, &[])).unwrap();
    let driver = r#"
import os, pty, select, signal, sys, time
pid, fd = pty.fork()
if pid == 0:
    os.environ['HOME'] = sys.argv[2]
    os.execvp('bash', ['bash', '--noprofile', '--rcfile', sys.argv[1], '-i'])
def wait_for(marker):
    output = b''
    deadline = time.monotonic() + 5
    while marker not in output:
        assert time.monotonic() < deadline, repr(output)
        if select.select([fd], [], [], .1)[0]:
            output += os.read(fd, 4096)
    return output
try:
    wait_for(b'__READY__')
    os.write(fd, b'\x03')
    time.sleep(.1)
    os.write(fd, b"printf '__PROMPT_%s__\\n' ALIVE\n")
    wait_for(b'__PROMPT_ALIVE__')
finally:
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    os.close(fd)
"#;
    let result = Command::new("python3")
        .args([
            "-c",
            driver,
            rc.to_str().unwrap(),
            home.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!dir.path().join("should-not-exist").exists());
}

#[cfg(unix)]
#[test]
fn preserves_array_prompt_hooks_and_runs_startup_only_once() {
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join(".bashrc"),
        "PROMPT_COMMAND=('printf OLD_PROMPT')\n",
    )
    .unwrap();
    let rc = dir.path().join("rc");
    let pane = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: dir.path().to_str().unwrap().into(),
        commands: vec!["printf x >> count".into()],
        shell: Shell::Bash,
    };
    fs::write(&rc, pane_script(&pane, &[])).unwrap();
    let mut child = Command::new("bash")
        .args(["--noprofile", "--rcfile", rc.to_str().unwrap(), "-i"])
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"true\ntrue\nexit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("OLD_PROMPT"));
    assert_eq!(fs::read_to_string(dir.path().join("count")).unwrap(), "x");
}
#[test]
fn tabs_args_open_one_tab_per_project() {
    let split = Layout::Split {
        id: "s".into(),
        axis: Axis::Columns,
        ratio: 0.5,
        first: Box::new(pane("a")),
        second: Box::new(pane("b")),
    };
    let bash = |ids: &[&str], folder: &str| -> BTreeMap<String, PaneLaunch> {
        ids.iter()
            .map(|id| {
                (
                    id.to_string(),
                    PaneLaunch::Bash(format!("/tmp/runterm-{folder}/{id}.sh")),
                )
            })
            .collect()
    };
    // Both projects use the same template, hence the same pane ids.
    let (first, second) = (bash(&["a", "b"], "one"), bash(&["a", "b"], "two"));
    let args = tabs_args(
        "new",
        &[
            TabLaunch {
                layout: &split,
                distribution: "Ubuntu",
                profile: "Ubuntu",
                launches: &first,
            },
            TabLaunch {
                layout: &split,
                distribution: "Debian",
                profile: "Sombre",
                launches: &second,
            },
        ],
    )
    .unwrap();
    assert_eq!(&args[..4], ["-w", "new", "--maximized", "new-tab"]);
    assert_eq!(args.iter().filter(|a| *a == "new-tab").count(), 2);
    assert_eq!(args.iter().filter(|a| *a == "split-pane").count(), 2);
    assert_eq!(args[args.len() - 4..], [";", "focus-tab", "--target", "0"]);
    let second_tab = args.iter().rposition(|a| a == "new-tab").unwrap();
    assert_eq!(args[second_tab - 1], ";");
    let tail = &args[second_tab..];
    assert!(tail.windows(2).any(|a| a == ["--distribution", "Debian"]));
    assert!(tail.windows(2).any(|a| a == ["--profile", "Sombre"]));
    assert!(tail.iter().any(|a| a.contains("/tmp/runterm-two/a.sh")));
    assert!(tail.iter().all(|a| !a.contains("runterm-one")));
    // A single tab keeps the historical output, without focus-tab.
    assert_eq!(
        tabs_args(
            "new",
            &[TabLaunch {
                layout: &split,
                distribution: "Ubuntu",
                profile: "Ubuntu",
                launches: &first,
            }]
        )
        .unwrap(),
        terminal_args(&split, "Ubuntu", "Ubuntu", &first).unwrap()
    );
    assert!(terminal_args(&split, "Ubuntu", "Ubuntu", &first)
        .unwrap()
        .iter()
        .all(|a| a != "focus-tab"));
    assert!(tabs_args("new", &[]).is_err());
    let (one, single) = (bash(&["a"], "x"), pane("a"));
    let many = vec![
        TabLaunch {
            layout: &single,
            distribution: "",
            profile: "",
            launches: &one,
        };
        MAX_LAUNCH_PANES + 1
    ];
    assert!(tabs_args("new", &many[..MAX_LAUNCH_PANES]).is_ok());
    assert!(tabs_args("new", &many).is_err());
}
#[test]
fn launch_groups_are_optional_and_validated() {
    let mut c = config();
    let json = serde_json::to_value(&c).unwrap();
    assert!(json.get("launchGroups").is_none());
    c.launch_groups.push(LaunchGroup {
        id: "g".into(),
        name: "Boulot".into(),
        windows: BTreeMap::from([("p".into(), 2)]),
        project_ids: Vec::new(),
    });
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["launchGroups"][0]["windows"]["p"], 2);
    assert_eq!(serde_json::from_value::<Config>(json).unwrap(), c);
    assert!(c.validate().is_ok());
    c.launch_groups[0].windows.insert("absent".into(), 1);
    assert!(c.validate().unwrap_err().contains("projet absent"));
    c.launch_groups[0].windows.remove("absent");
    for window in [0, MAX_LAUNCH_WINDOWS + 1] {
        c.launch_groups[0].windows.insert("p".into(), window);
        assert!(c.validate().unwrap_err().contains("de 1 à"));
    }
    c.launch_groups[0]
        .windows
        .insert("p".into(), MAX_LAUNCH_WINDOWS);
    assert!(c.validate().is_ok());
    c.launch_groups[0].name = " ".into();
    assert!(c.validate().is_err());
    c.launch_groups[0].name = "Boulot".into();
    c.launch_groups.push(c.launch_groups[0].clone());
    assert!(c.validate().unwrap_err().contains("dupliqué"));
}
#[test]
fn load_moves_launch_all_into_a_group() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("config.json");
    let mut json = serde_json::to_value(config()).unwrap();
    json["projects"][0]["launchAll"] = true.into();
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let loaded = load(&path).unwrap().unwrap();
    assert_eq!(loaded.launch_groups.len(), 1);
    assert_eq!(loaded.launch_groups[0].name, "Principal");
    assert_eq!(
        loaded.launch_groups[0].windows,
        BTreeMap::from([("p".into(), 1)])
    );
    assert!(!loaded.projects[0].launch_all);
    // The flag is never written back.
    let saved = serde_json::to_value(&loaded).unwrap();
    assert!(saved["projects"][0].get("launchAll").is_none());
    // Existing groups win over leftover flags.
    json["launchGroups"] = serde_json::json!([{"id": "g", "name": "Perso", "windows": {}}]);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let loaded = load(&path).unwrap().unwrap();
    assert_eq!(loaded.launch_groups.len(), 1);
    assert_eq!(loaded.launch_groups[0].name, "Perso");
    // The flat selection of the first groups lands in window 1, never written back.
    json["launchGroups"] = serde_json::json!([{"id": "g", "name": "Perso", "projectIds": ["p"]}]);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let loaded = load(&path).unwrap().unwrap();
    assert_eq!(
        loaded.launch_groups[0].windows,
        BTreeMap::from([("p".into(), 1)])
    );
    let saved = serde_json::to_value(&loaded).unwrap();
    assert!(saved["launchGroups"][0].get("projectIds").is_none());
}

#[test]
fn host_is_optional_and_validated() {
    let mut c = config();
    let json = serde_json::to_value(&c).unwrap();
    assert!(json["projects"][0].get("host").is_none());
    for good in ["vps", "me@dev-vm.example.com", "me@10.0.0.2", "::1"] {
        c.projects[0].host = good.into();
        assert!(c.validate().is_ok(), "{good}");
    }
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["projects"][0]["host"], "::1");
    assert_eq!(serde_json::from_value::<Config>(json).unwrap(), c);
    for bad in [
        "-oProxyCommand=x",
        "a b",
        "a;b",
        "a'b",
        "a\"b",
        "h\n",
        "$(x)",
    ] {
        c.projects[0].host = bad.into();
        assert!(c.validate().is_err(), "{bad:?}");
        assert!(ssh_command(bad, "").is_err());
    }
    assert!(ssh_command("", "").is_err());
}
#[test]
fn ssh_panes_run_ssh_from_wsl() {
    let l = split(Axis::Columns, pane("a"), pane("b"));
    let launches = BTreeMap::from([
        (
            "a".into(),
            PaneLaunch::Ssh {
                host: "me@vm".into(),
                script: "echo 'a ; \"b\"'".into(),
            },
        ),
        (
            "b".into(),
            PaneLaunch::Ssh {
                host: "me@vm".into(),
                script: String::new(),
            },
        ),
    ]);
    let args = terminal_args(&l, "Ubuntu", "Ubuntu", &launches).unwrap();
    let command = ssh_command("me@vm", "echo 'a ; \"b\"'").unwrap();
    assert!(args.windows(11).any(|a| a
        == [
            "--profile",
            "Ubuntu",
            "--title",
            "a",
            "--",
            "wsl.exe",
            "--distribution",
            "Ubuntu",
            "--exec",
            "bash",
            "-lic",
        ]));
    assert!(args.contains(&command));
    // Windows Terminal splits on `;` and may not relay `"` to wsl.exe.
    assert!(args.iter().all(|a| a == ";" || !a.contains([';', '"'])));
    assert_eq!(simulate_windows_terminal(&args), "-V 0.500000(a,b) focus=a");
}
/// Runs `ssh_command` with a fake `ssh` that hands the remote command to
/// `sh -c`, as sshd does with the user's login shell.
#[cfg(unix)]
#[test]
fn ssh_command_runs_the_pane_script_on_the_host() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    let d = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("ssh");
    fs::write(
        &fake,
        "#!/bin/sh\n[ \"$1 $2 $3\" = '-t -- me@vm' ] || exit 99\nexec sh -c \"$4\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let output_file = d.path().join("result ; 'quoted'.txt");
    let pane = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: d.path().to_str().unwrap().into(),
        commands: vec![
            "export RUNTERM_TEST='hello ; \"world\"'".into(),
            format!(
                "printf '%s\\n' \"$RUNTERM_TEST $PWD\" > {}",
                shell_quote(output_file.to_str().unwrap())
            ),
        ],
        shell: Shell::Bash,
    };
    let mut child = Command::new("bash")
        .args([
            "--noprofile",
            "--norc",
            "-c",
            &ssh_command("me@vm", &pane_script(&pane, &[])).unwrap(),
        ])
        .env("HOME", home.path())
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"printf 'PROMPT-%s\\n' \"$PWD\"\nexit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(&format!("PROMPT-{}", d.path().display())),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(output_file).unwrap(),
        format!("hello ; \"world\" {}\n", d.path().display())
    );
}

/// Values a user may type in names and paths, hostile to every layer a pane
/// command crosses (wt.exe, wsl.exe or ssh, then Bash).
const TRAPPED: &str = "l'été ; $HOME \"dq\" `id` $(id) \\ & | > é";

fn trapped_project() -> Project {
    let mut project = config().projects.remove(0);
    project.id = format!("id {TRAPPED}");
    project.name = format!("Nom {TRAPPED}");
    project.root = format!("/home/me/wt 4/{TRAPPED}");
    project
}

fn info() -> LaunchInfo<'static> {
    LaunchInfo {
        launch_id: "0b6f1c1e-2f6a-4c55-9d38-2a7c0d1e9f10",
        launch_group: "groupe 'g'",
        window_number: 3,
        window_name: "runterm-0b6f1c1e-3",
        tab: 2,
    }
}

/// `RUNTERM_*` lines of `env`, sorted, as `NAME=value`.
fn runterm_env(output: &str) -> Vec<String> {
    let mut lines: Vec<String> = output
        .split('\0')
        .filter(|l| l.starts_with("RUNTERM_"))
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

fn expected_env(env: &[(&str, String)]) -> Vec<String> {
    let mut lines: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
    lines.sort();
    lines
}

#[test]
fn pane_order_is_windows_terminal_creation_order() {
    use Axis::{Columns as C, Rows as R};
    let layouts = [
        pane("a"),
        split(C, pane("a"), pane("b")),
        split(R, split(C, pane("a"), pane("b")), pane("c")),
        split(
            R,
            split(C, pane("a"), pane("b")),
            split(C, pane("c"), split(C, pane("d"), pane("e"))),
        ),
        split(
            C,
            pane("a"),
            split(R, split(C, pane("b"), pane("c")), pane("d")),
        ),
    ];
    for l in &layouts {
        let launches = l
            .panes()
            .iter()
            .map(|p| {
                (
                    p.id().to_owned(),
                    PaneLaunch::Bash(format!("/tmp/runterm-t/{}.sh", p.id())),
                )
            })
            .collect();
        let args = terminal_args(l, "Ubuntu", "Ubuntu", &launches).unwrap();
        // Titles (the pane ids here) of `new-tab` and each `split-pane`: the
        // panes in the order Windows Terminal creates and numbers them.
        let created: Vec<&str> = args
            .split(|a| a == ";")
            .filter(|c| c.iter().any(|a| a == "new-tab" || a == "split-pane"))
            .map(|c| c[c.iter().position(|a| a == "--title").unwrap() + 1].as_str())
            .collect();
        assert_eq!(pane_order(l), created);
    }
    // Not the reading order of the tree.
    assert_eq!(
        pane_order(&layouts[3]),
        ["a", "c", "b", "d", "e"],
        "{:?}",
        layouts[3]
    );
}

#[test]
fn pane_env_describes_the_pane_of_a_multi_pane_tab() {
    use Axis::{Columns as C, Rows as R};
    let mut c = config();
    c.templates[0].layout = split(
        R,
        split(C, pane("a"), pane("b")),
        split(C, pane("c"), pane("d")),
    );
    let (project, template, panes) = c.resolve("p").unwrap();
    let env: Vec<Vec<(&str, String)>> = panes
        .iter()
        .map(|p| pane_env(&info(), project, "Ubuntu", &template.layout, p))
        .collect();
    let get = |pane: usize, name: &str| {
        env[pane]
            .iter()
            .find(|(k, _)| *k == name)
            .unwrap()
            .1
            .clone()
    };
    // a, b, c, d in reading order; created a, c, b, d.
    let indexes: Vec<String> = (0..4).map(|i| get(i, "RUNTERM_PANE_INDEX")).collect();
    assert_eq!(indexes, ["0", "2", "1", "3"]);
    assert_eq!(get(1, "RUNTERM_PANE_ID"), "b");
    assert_eq!(get(1, "RUNTERM_PANE_NAME"), "b");
    assert_eq!(
        expected_env(&env[0]),
        expected_env(&[
            ("RUNTERM_ENV_VERSION", "1".into()),
            (
                "RUNTERM_LAUNCH_ID",
                "0b6f1c1e-2f6a-4c55-9d38-2a7c0d1e9f10".into()
            ),
            ("RUNTERM_PROJECT_ID", "p".into()),
            ("RUNTERM_PROJECT_NAME", "Projet".into()),
            ("RUNTERM_PROJECT_ROOT", "/home/me/project".into()),
            ("RUNTERM_HOST", String::new()),
            ("RUNTERM_DISTRIBUTION", "Ubuntu".into()),
            ("RUNTERM_LAUNCH_GROUP", "groupe 'g'".into()),
            ("RUNTERM_TEMPLATE_ID", "t".into()),
            ("RUNTERM_WINDOW_NUMBER", "3".into()),
            ("RUNTERM_WINDOW_NAME", "runterm-0b6f1c1e-3".into()),
            ("RUNTERM_TAB", "2".into()),
            ("RUNTERM_PANE_INDEX", "0".into()),
            ("RUNTERM_PANE_ID", "a".into()),
            ("RUNTERM_PANE_NAME", "a".into()),
        ])
    );
    // On an SSH host the panes do not run in a WSL distribution.
    let mut remote = project.clone();
    remote.host = "me@vm".into();
    let ssh = pane_env(&info(), &remote, "Ubuntu", &template.layout, &panes[0]);
    assert!(ssh.contains(&("RUNTERM_HOST", "me@vm".into())));
    assert!(ssh.contains(&("RUNTERM_DISTRIBUTION", String::new())));
}

/// Runs `script` as the rcfile of an interactive Bash fed `input`.
#[cfg(unix)]
fn run_rcfile(script: &str, input: &str) -> std::process::Output {
    use std::process::{Command, Stdio};
    let d = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let rc = d.path().join("rc");
    fs::write(&rc, script).unwrap();
    let mut child = Command::new("bash")
        .args(["--noprofile", "--rcfile", rc.to_str().unwrap(), "-i"])
        .env("HOME", home.path())
        .env("PS1", "$ ")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[cfg(unix)]
#[test]
fn local_pane_exports_trapped_values_before_its_actions() {
    let d = tempfile::tempdir().unwrap();
    let directory = d.path().join(TRAPPED);
    fs::create_dir(&directory).unwrap();
    let output_file = d.path().join("env");
    let project = trapped_project();
    let pane = ResolvedPane {
        id: format!("pane {TRAPPED}"),
        name: format!("AI {TRAPPED}"),
        directory: directory.to_str().unwrap().into(),
        // An action sees them, and so does every program it starts.
        commands: vec![format!(
            "env -0 > {}",
            shell_quote(output_file.to_str().unwrap())
        )],
        shell: Shell::Bash,
    };
    let layout = Layout::Pane {
        id: pane.id.clone(),
        name: pane.name.clone(),
        directory: ".".into(),
        commands: Vec::new(),
        shell: Shell::Bash,
    };
    let env = pane_env(&info(), &project, "Ubuntu", &layout, &pane);
    let out = run_rcfile(&pane_script(&pane, &env), "exit\n");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let seen = runterm_env(&fs::read_to_string(output_file).unwrap());
    assert_eq!(seen, expected_env(&env));
    assert!(seen.contains(&format!("RUNTERM_PROJECT_ROOT=/home/me/wt 4/{TRAPPED}")));
    assert!(seen.contains(&"RUNTERM_PANE_INDEX=0".to_owned()));
}

/// A pane without action is still a silent interactive shell in its folder:
/// the variables add no output.
#[cfg(unix)]
#[test]
fn pane_without_actions_stays_a_silent_shell() {
    let d = tempfile::tempdir().unwrap();
    let resolved = ResolvedPane {
        id: "a".into(),
        name: "a".into(),
        directory: d.path().to_str().unwrap().into(),
        commands: Vec::new(),
        shell: Shell::Bash,
    };
    let project = trapped_project();
    let env = pane_env(&info(), &project, "Ubuntu", &pane("a"), &resolved);
    let input = "printf 'PWD=%s\\n' \"$PWD\"\nprintf '%s\\n' \"$RUNTERM_PANE_NAME\"\nexit\n";
    let with = run_rcfile(&pane_script(&resolved, &env), input);
    let without = run_rcfile(&pane_script(&resolved, &[]), input);
    // Only the echoed variable differs; anything else printed (by the
    // system bashrc, for instance) is the same with and without variables.
    let (with_out, without_out) = (
        String::from_utf8_lossy(&with.stdout),
        String::from_utf8_lossy(&without.stdout),
    );
    let pwd = format!("PWD={}\n", d.path().display());
    assert!(with_out.ends_with(&format!("{pwd}a\n")), "{with_out}");
    assert_eq!(
        with_out.replace(&format!("{pwd}a\n"), &format!("{pwd}\n")),
        without_out
    );
    assert_eq!(with.stderr, without.stderr);
}

/// Same as `ssh_command_runs_the_pane_script_on_the_host`, for the variables:
/// they travel inside the script, not through `SendEnv` or `WSLENV`.
#[cfg(unix)]
#[test]
fn ssh_pane_exports_trapped_values_on_the_host() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    let d = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("ssh");
    // A remote login starts from a clean environment.
    fs::write(
        &fake,
        "#!/bin/sh\n[ \"$1 $2 $3\" = '-t -- me@vm' ] || exit 99\nexec env -i HOME=\"$HOME\" PATH=\"$PATH\" sh -c \"$4\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let output_file = d.path().join("env");
    let mut project = trapped_project();
    project.host = "me@vm".into();
    let resolved = ResolvedPane {
        id: "a".into(),
        name: format!("AI {TRAPPED}"),
        directory: d.path().to_str().unwrap().into(),
        commands: vec![format!(
            "env -0 > {}",
            shell_quote(output_file.to_str().unwrap())
        )],
        shell: Shell::Bash,
    };
    let layout = split(Axis::Columns, pane("z"), pane("a"));
    let env = pane_env(&info(), &project, "Ubuntu", &layout, &resolved);
    let command = ssh_command("me@vm", &pane_script(&resolved, &env)).unwrap();
    // What Windows Terminal receives: no `;` it would split on, no `"`.
    assert!(!command.contains([';', '"']));
    let mut child = Command::new("bash")
        .args(["--noprofile", "--norc", "-c", &command])
        .env("HOME", home.path())
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    let out = child.wait_with_output().unwrap();
    let seen = runterm_env(&fs::read_to_string(&output_file).unwrap_or_else(|_| {
        panic!("{}", String::from_utf8_lossy(&out.stderr));
    }));
    assert_eq!(seen, expected_env(&env));
    assert!(seen.contains(&"RUNTERM_HOST=me@vm".to_owned()));
    assert!(seen.contains(&"RUNTERM_DISTRIBUTION=".to_owned()));
    assert!(seen.contains(&"RUNTERM_PANE_INDEX=1".to_owned()));
}

#[test]
fn powershell_pane_sets_quoted_variables_first() {
    let pane = ResolvedPane {
        id: "a".into(),
        name: "l'été ‘x’".into(),
        directory: String::new(),
        commands: vec!["echo ok".into()],
        shell: Shell::Powershell,
    };
    let env = vec![
        ("RUNTERM_PANE_NAME", pane.name.clone()),
        ("RUNTERM_TAB", "1".into()),
    ];
    let script = powershell_script(&pane, r"\\wsl.localhost\Ubuntu\p", &env);
    let lines: Vec<&str> = script.lines().collect();
    assert_eq!(lines[1], "$env:RUNTERM_PANE_NAME = 'l''été ‘‘x’’'");
    assert_eq!(lines[2], "$env:RUNTERM_TAB = '1'");
    assert!(lines[3].starts_with("$__runterm_ok"));
}

#[test]
fn tabs_args_name_the_window() {
    let one = BTreeMap::from([(
        "a".to_owned(),
        PaneLaunch::Bash("/tmp/runterm-x/a.sh".into()),
    )]);
    let single = pane("a");
    let tab = TabLaunch {
        layout: &single,
        distribution: "Ubuntu",
        profile: "Ubuntu",
        launches: &one,
    };
    let args = tabs_args("runterm-0b6f1c1e-2", &[tab, tab]).unwrap();
    assert_eq!(
        &args[..4],
        ["-w", "runterm-0b6f1c1e-2", "--maximized", "new-tab"]
    );
    for bad in ["", "12", "-1", "a b", "a;b", "runterm_1", "fenêtre"] {
        assert!(tabs_args(bad, &[tab]).is_err(), "{bad}");
    }
    assert!(valid_window_name("new") && valid_window_name("runterm-1"));
}
