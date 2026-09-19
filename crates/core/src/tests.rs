use super::*;
fn pane(id: &str) -> Layout {
    Layout::Pane {
        id: id.into(),
        name: id.into(),
        directory: ".".into(),
        commands: vec!["echo ok".into()],
    }
}
fn config() -> Config {
    Config {
        schema_version: 1,
        templates: vec![Template {
            id: "t".into(),
            name: "Dev".into(),
            layout: pane("a"),
        }],
        projects: vec![Project {
            id: "p".into(),
            name: "Projet".into(),
            root: "/home/me/project".into(),
            distribution: "Ubuntu".into(),
            terminal_profile: String::new(),
            template_id: "t".into(),
            overrides: BTreeMap::new(),
        }],
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
        .map(|id| (id.into(), format!("/tmp/runterm-test/{id}.sh")))
        .into();
    let args = terminal_args(&l, "Ubuntu", "Ubuntu", &scripts).unwrap();
    assert_eq!(args.iter().filter(|a| *a == "split-pane").count(), 2);
    assert!(args.windows(3).any(|a| a == ["-H", "--size", "0.600000"]));
    assert!(args.windows(3).any(|a| a == ["-V", "--size", "0.300000"]));
    assert!(args.iter().any(|a| a == "previousInOrder"));
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
    fn leaves(n: &Node, out: &mut Vec<String>) {
        match n {
            Node::Leaf(t) => out.push(t.clone()),
            Node::Split(_, _, a, b) => {
                leaves(a, out);
                leaves(b, out);
            }
        }
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
    let (mut root, mut focused) = (None::<Node>, String::new());
    for command in args.split(|a| a == ";") {
        match command
            .iter()
            .position(|a| a == "new-tab" || a == "split-pane" || a == "move-focus")
        {
            Some(i) if command[i] == "new-tab" => {
                focused = title(command);
                root = Some(Node::Leaf(focused.clone()));
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
                focused = new;
            }
            Some(i) => {
                let mut order = Vec::new();
                leaves(root.as_ref().unwrap(), &mut order);
                let at = order.iter().position(|t| *t == focused).unwrap();
                let len = order.len();
                focused = match command[i + 1].as_str() {
                    "nextInOrder" => order[(at + 1) % len].clone(),
                    "previousInOrder" => order[(at + len - 1) % len].clone(),
                    other => panic!("direction inattendue {other}"),
                };
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
            .map(|id| (id.clone(), format!("/tmp/runterm-test/{id}.sh")))
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
    };
    let rc = d.path().join("rc");
    fs::write(&rc, pane_script(&p)).unwrap();
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
            "printf '__READY__\\n'; sleep 30".into(),
            "touch should-not-exist".into(),
        ],
    };
    fs::write(&rc, pane_script(&pane)).unwrap();
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
    };
    fs::write(&rc, pane_script(&pane)).unwrap();
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
