use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Layout {
    Pane {
        id: String,
        name: String,
        directory: String,
        commands: Vec<String>,
        #[serde(default, skip_serializing_if = "Shell::is_bash")]
        shell: Shell,
    },
    Split {
        id: String,
        axis: Axis,
        ratio: f64,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    Columns,
    Rows,
}
/// Program running a pane's actions. Absent from the JSON: Bash.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Shell {
    /// Bash in the project's WSL distribution.
    #[default]
    Bash,
    /// PowerShell on Windows, started in the project folder through `\\wsl.localhost`.
    Powershell,
}
impl Shell {
    fn is_bash(&self) -> bool {
        *self == Self::Bash
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub layout: Layout,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Override {
    pub directory: Option<String>,
    pub commands: Option<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root: String,
    /// SSH destination (`user@host` or a `~/.ssh/config` alias) the panes
    /// connect to with the `ssh` of `distribution`; `root` is then a folder
    /// on that host. Empty: the panes run in WSL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
    pub distribution: String,
    /// Windows Terminal profile (name or GUID) giving the panes their look.
    /// Empty: the profile named after the distribution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub terminal_profile: String,
    /// Page opened in the default browser when the project is launched, for a
    /// web application served by one of its panes. Empty: none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// Keeps `url` in the configuration without opening it at launch, for when
    /// the browser is not wanted for a while.
    #[serde(default, skip_serializing_if = "is_false")]
    pub url_disabled: bool,
    /// Opens the project in Visual Studio Code at launch, connected to WSL (or
    /// to `host` over SSH) with its Remote extensions.
    #[serde(default, skip_serializing_if = "is_false")]
    pub vscode: bool,
    /// Folder VS Code opens: relative to `root`, or absolute. Empty: `root`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vscode_folder: String,
    pub template_id: String,
    pub overrides: BTreeMap<String, Override>,
    /// Started by the "launch all" buttons.
    #[serde(default, skip_serializing_if = "is_false")]
    pub launch_all: bool,
}
impl Project {
    /// Page to open in the browser when the project is launched. Empty when
    /// the project has no URL, and when `url_disabled` keeps the value the
    /// user typed without opening it.
    pub fn url_to_open(&self) -> &str {
        if self.url_disabled {
            ""
        } else {
            &self.url
        }
    }
    /// `--folder-uri` value opening the project in VS Code, e.g.
    /// `vscode-remote://wsl+Ubuntu/home/me/project`, or
    /// `vscode-remote://ssh-remote+dev/srv/app` with a `host`. `distribution`
    /// is the one the panes use, resolved when the project names none. Empty
    /// when the project does not open VS Code.
    pub fn vscode_uri(&self, distribution: &str) -> String {
        if !self.vscode {
            return String::new();
        }
        let folder = match self.vscode_folder.as_str() {
            "" | "." | "./" => self.root.clone(),
            absolute if absolute.starts_with('/') => absolute.to_owned(),
            relative => format!("{}/{relative}", self.root.trim_end_matches('/')),
        };
        let folder = match folder.trim_end_matches('/') {
            "" => "/",
            trimmed => trimmed,
        };
        let authority = if self.host.is_empty() {
            format!("wsl+{}", percent_encode(distribution, ""))
        } else {
            format!("ssh-remote+{}", percent_encode(&self.host, ""))
        };
        format!("vscode-remote://{authority}{}", percent_encode(folder, "/"))
    }
}
/// Percent-encodes every byte of `value` but the URI unreserved characters
/// and `keep`, so VS Code reads it back unchanged (`@` or `:` in a host would
/// otherwise split the authority).
fn percent_encode(value: &str, keep: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || b"-._~".contains(&byte)
            || keep.as_bytes().contains(&byte)
        {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
fn is_false(value: &bool) -> bool {
    !value
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    /// WSL folder holding the user's projects: prefills new project roots and
    /// lists their subfolders. Empty: not set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub workspace_root: String,
    pub templates: Vec<Template>,
    pub projects: Vec<Project>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPane {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub commands: Vec<String>,
    pub shell: Shell,
}

impl Layout {
    pub fn panes(&self) -> Vec<&Layout> {
        match self {
            Self::Pane { .. } => vec![self],
            Self::Split { first, second, .. } => {
                let mut p = first.panes();
                p.extend(second.panes());
                p
            }
        }
    }
    pub fn id(&self) -> &str {
        match self {
            Self::Pane { id, .. } | Self::Split { id, .. } => id,
        }
    }
    fn validate(&self, ids: &mut HashSet<String>, depth: usize) -> Result<(), String> {
        if depth > 20 {
            return Err("Layout trop profondément imbriqué.".into());
        }
        valid_text(self.id(), "Identifiant")?;
        if !ids.insert(self.id().to_owned()) {
            return Err("Identifiant de panneau dupliqué.".into());
        }
        match self {
            Self::Pane {
                name,
                directory,
                commands,
                ..
            } => {
                valid_text(name, "Nom du panneau")?;
                valid_directory(directory)?;
                if commands.iter().any(|c| c.contains('\0')) {
                    return Err("Commande contenant un caractère nul.".into());
                }
            }
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                    return Err("La proportion doit être comprise entre 10 et 90 %.".into());
                }
                first.validate(ids, depth + 1)?;
                second.validate(ids, depth + 1)?;
            }
        }
        Ok(())
    }
}
fn valid_text(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.contains(['\0', '\r', '\n', ';']) {
        return Err(format!(
            "{field} vide ou contenant un caractère interdit (retour à la ligne, point-virgule)."
        ));
    }
    Ok(())
}
/// Kept to characters `ssh` reads as a destination and that no shell or
/// Windows Terminal interprets; a leading `-` would be an option.
fn valid_host(value: &str) -> Result<(), String> {
    if value.starts_with('-')
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-@:".contains(&c))
    {
        return Err("Hôte SSH invalide (lettres, chiffres et « . _ - @ : » uniquement).".into());
    }
    Ok(())
}
/// Kept to an absolute `http(s)` address, with no character a browser could
/// read as an option and no space, so the URL stays one argument.
pub fn valid_url(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or("L’URL du projet doit commencer par http:// ou https://.")?;
    if rest.is_empty()
        || value.len() > 2000
        || value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "\"'<>|".contains(c))
    {
        return Err("URL du projet invalide (espace, guillemet ou caractère de contrôle).".into());
    }
    Ok(())
}
fn valid_directory(value: &str) -> Result<(), String> {
    if value.starts_with('/') || value.contains('\0') || value.split('/').any(|s| s == "..") {
        return Err("Le dossier du panneau doit être relatif à la racine, sans « .. ».".into());
    }
    Ok(())
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Version du fichier de configuration non prise en charge.".into());
        }
        if !self.workspace_root.is_empty()
            && (!self.workspace_root.starts_with('/') || self.workspace_root.contains('\0'))
        {
            return Err("La racine des workspaces doit être un chemin Linux absolu.".into());
        }
        let mut ids = HashSet::new();
        for template in &self.templates {
            valid_text(&template.id, "Identifiant du modèle")?;
            valid_text(&template.name, "Nom du modèle")?;
            if !ids.insert(&template.id) {
                return Err("Identifiant de modèle dupliqué.".into());
            }
            template.layout.validate(&mut HashSet::new(), 0)?;
            if template.layout.panes().len() > 16 {
                return Err("Un layout peut contenir au maximum 16 panneaux.".into());
            }
        }
        let mut projects = HashSet::new();
        for project in &self.projects {
            valid_text(&project.id, "Identifiant du projet")?;
            valid_text(&project.name, "Nom du projet")?;
            if !projects.insert(&project.id) {
                return Err("Identifiant de projet dupliqué.".into());
            }
            if !ids.contains(&project.template_id) {
                return Err("Un projet référence un modèle absent.".into());
            }
            if !project.root.starts_with('/') || project.root.contains('\0') {
                return Err("La racine doit être un chemin Linux absolu.".into());
            }
            valid_host(&project.host)?;
            valid_url(&project.url)?;
            if project.vscode_folder.chars().any(char::is_control) {
                return Err("Dossier VS Code invalide (caractère de contrôle).".into());
            }
            if project.distribution.contains(['\0', '\r', '\n', ';']) {
                return Err("Distribution invalide.".into());
            }
            if project.terminal_profile.contains(['\0', '\r', '\n', ';'])
                || project.terminal_profile.trim_start().starts_with('-')
            {
                return Err("Profil Windows Terminal invalide.".into());
            }
            for value in project.overrides.values() {
                if let Some(dir) = &value.directory {
                    valid_directory(dir)?;
                }
                if value
                    .commands
                    .as_ref()
                    .is_some_and(|c| c.iter().any(|v| v.contains('\0')))
                {
                    return Err("Commande contenant un caractère nul.".into());
                }
            }
        }
        Ok(())
    }
    pub fn resolve(
        &self,
        project_id: &str,
    ) -> Result<(&Project, &Template, Vec<ResolvedPane>), String> {
        self.validate()?;
        let project = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or("Projet introuvable.")?;
        let template = self
            .templates
            .iter()
            .find(|t| t.id == project.template_id)
            .ok_or("Modèle introuvable.")?;
        let panes = template
            .layout
            .panes()
            .iter()
            .map(|p| {
                if let Layout::Pane {
                    id,
                    name,
                    directory,
                    commands,
                    shell,
                } = p
                {
                    let custom = project.overrides.get(id);
                    let relative = custom
                        .and_then(|c| c.directory.as_ref())
                        .unwrap_or(directory);
                    ResolvedPane {
                        id: id.clone(),
                        name: name.clone(),
                        directory: format!(
                            "{}/{}",
                            project.root.trim_end_matches('/'),
                            if relative.is_empty() { "." } else { relative }
                        ),
                        commands: custom
                            .and_then(|c| c.commands.clone())
                            .unwrap_or_else(|| commands.clone()),
                        shell: *shell,
                    }
                } else {
                    unreachable!()
                }
            })
            .collect();
        Ok((project, template, panes))
    }
}

pub fn load(path: &Path) -> Result<Option<Config>, String> {
    match fs::read(path) {
        Ok(bytes) => {
            let config: Config = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Configuration illisible, fichier conservé : {e}"))?;
            config.validate()?;
            Ok(Some(config))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Lecture impossible : {e}")),
    }
}
pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    config.validate()?;
    // Refuse to replace an existing malformed or future-version configuration.
    load(path)?;
    let parent = path.parent().ok_or("Dossier de configuration absent.")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// An interactive rcfile: actions share the shell, and failure/INT returns to its prompt.
pub fn pane_script(pane: &ResolvedPane) -> String {
    let mut script =
        String::from("# Generated by RunTerm\n[[ -f ~/.bashrc ]] && source ~/.bashrc\n");
    script.push_str("__runterm_actions() {\n    trap 'return 130' INT\n");
    script.push_str(&format!("    if ! builtin cd -- {}; then\n      printf '%s\\n' 'RunTerm : dossier inaccessible.' >&2\n      return 1\n    fi\n", shell_quote(&pane.directory)));
    for (index, command) in pane.commands.iter().enumerate() {
        if command.trim().is_empty() {
            continue;
        }
        // The action is pushed on the history list so the up arrow recalls it,
        // as if the user had typed it. Trimmed, because a leading space keeps
        // the entry out of the history under the common `HISTCONTROL=ignoreboth`.
        script.push_str(&format!(
            "    builtin history -s -- {}\n",
            shell_quote(command.trim())
        ));
        script.push_str(&format!("    builtin eval -- {}\n    local __rt_status=$?\n    if (( __rt_status != 0 )); then\n      printf 'RunTerm : action {} interrompue (code %s).\\n' \"$__rt_status\" >&2\n      return \"$__rt_status\"\n    fi\n", shell_quote(command), index + 1));
    }
    // Run only once, after Bash has completed initialization and enabled job control.
    // Restore scalar/array PROMPT_COMMAND before actions so user prompts keep working.
    script.push_str(
        r#"    return 0
}
__runterm_old_prompt=$(declare -p PROMPT_COMMAND 2>/dev/null)
__runterm_startup() {
    unset PROMPT_COMMAND
    if [[ -n $__runterm_old_prompt ]]; then
        builtin eval -- "${__runterm_old_prompt/declare /declare -g }"
    fi
    unset __runterm_old_prompt
    local __runterm_old_int=$(trap -p INT)
    __runterm_actions
    trap - INT
    [[ -n $__runterm_old_int ]] && builtin eval -- "$__runterm_old_int"
    unset -f __runterm_actions __runterm_startup
}
PROMPT_COMMAND=__runterm_startup
"#,
    );
    script
}

/// PowerShell single-quoted literal. PowerShell also ends such literals on
/// typographic single quotes, so each of them is doubled too.
pub fn powershell_quote(value: &str) -> String {
    let mut quoted = String::from("'");
    for c in value.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
            quoted.push(c);
        }
        quoted.push(c);
    }
    quoted.push('\'');
    quoted
}

/// Windows path of a WSL folder, e.g. `\\wsl.localhost\Ubuntu\home\me`.
pub fn wsl_windows_path(distribution: &str, path: &str) -> String {
    format!(r"\\wsl.localhost\{distribution}{}", path.replace('/', r"\"))
}

/// Runs in the top-level scope of `powershell -NoExit`, so variables and
/// functions defined by an action stay available; the first failing action
/// (exception, `$?` false or non-zero `$LASTEXITCODE`) stops the sequence.
/// `directory` is a Windows path.
pub fn powershell_script(pane: &ResolvedPane, directory: &str) -> String {
    let mut script = format!(
        "# Generated by RunTerm\n$__runterm_ok = $true\ntry {{ Set-Location -LiteralPath {} -ErrorAction Stop }}\ncatch {{ $__runterm_ok = $false; [Console]::Error.WriteLine('RunTerm : dossier inaccessible.') }}\n",
        powershell_quote(directory)
    );
    for (index, command) in pane.commands.iter().enumerate() {
        if command.trim().is_empty() {
            continue;
        }
        let n = index + 1;
        script.push_str(&format!(
            "if ($__runterm_ok) {{\n    $global:LASTEXITCODE = 0\n    try {{\n        . ([scriptblock]::Create({}))\n        if (-not $? -or $global:LASTEXITCODE) {{\n            $__runterm_ok = $false\n            [Console]::Error.WriteLine(\"RunTerm : action {n} interrompue (code $global:LASTEXITCODE).\")\n        }}\n    }} catch {{\n        $__runterm_ok = $false\n        [Console]::Error.WriteLine(\"RunTerm : action {n} interrompue : $_\")\n    }}\n}}\n",
            powershell_quote(command)
        ));
    }
    script.push_str("Remove-Variable __runterm_ok\n");
    script
}

/// Value for `-EncodedCommand`: base64 of the UTF-16LE script.
fn encode_powershell(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

/// Standard base64. Its alphabet has no `;`, which Windows Terminal would read
/// as a command separator, and nothing a shell interprets.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// Bash command, run by `bash -lic` in WSL, connecting to `host` and running
/// `script` there as the rcfile of an interactive Bash, like [`pane_script`]
/// in WSL. The script travels in base64 inside the remote command, so nothing
/// is written on the host and authentication happens in the pane.
pub fn ssh_command(host: &str, script: &str) -> Result<String, String> {
    valid_host(host)?;
    if host.is_empty() {
        return Err("Hôte SSH absent.".into());
    }
    // Parsed by the user's login shell on the host (sh, bash, zsh or fish):
    // only single quotes, and no `"`, which Windows Terminal may not relay.
    let remote = format!(
        "exec bash -lc 'exec bash --rcfile <(base64 -d <<<$1) -i' runterm {}",
        base64(script.as_bytes())
    );
    Ok(format!(
        "exec ssh -t -- {} {}",
        shell_quote(host),
        shell_quote(&remote)
    ))
}

/// Value of the single entry printed by `reg.exe query … /v <name>` (or `/ve`):
/// a `    <name>    REG_SZ    <value>` line. The entry name is localized
/// (`(Default)`, `(Par défaut)`), its type is not, so the type is the anchor.
pub fn reg_string(output: &str) -> Option<String> {
    let value = output.lines().find_map(|line| {
        ["REG_EXPAND_SZ", "REG_SZ"]
            .iter()
            .find_map(|kind| line.split(kind).nth(1))
    })?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Program of a registry `shell\open\command` (a browser's, or the `vscode:`
/// handler naming `Code.exe`), e.g. `"C:\…\chrome.exe"
/// --single-argument %1` or `C:\…\firefox.exe -osint -url "%1"`. RunTerm
/// passes the URLs to that program itself instead of filling the command in:
/// browsers open one tab per URL argument, in a single window. `None` when
/// the value names no executable, for instance a Store application.
pub fn browser_executable(command: &str) -> Option<String> {
    let command = command.trim();
    let program = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        // Unquoted; such a path may still contain spaces, so it ends at `.exe`.
        None => {
            let end = command
                .as_bytes()
                .windows(4)
                .position(|w| w.eq_ignore_ascii_case(b".exe"))?
                + 4;
            &command[..end]
        }
    };
    let usable = program.len() > 4
        && program.as_bytes()[program.len() - 4..].eq_ignore_ascii_case(b".exe")
        && !program.contains(['%', '\0', '\r', '\n']);
    usable.then(|| program.to_owned())
}

/// Command line opening `urls` in a **new** window of `program`, the browser
/// [`browser_executable`] returned. Chromium-based browsers take
/// `--new-window`, and still gather every URL of one command line into that
/// single window, so projects launched together share it instead of landing in
/// the window the user already had open. Other browsers get the URLs alone:
/// none of their flags both forces a window and keeps the URLs together.
pub fn browser_args(program: &str, urls: &[String]) -> Vec<String> {
    const CHROMIUM: [&str; 6] = [
        "msedge.exe",
        "chrome.exe",
        "chromium.exe",
        "brave.exe",
        "vivaldi.exe",
        "opera.exe",
    ];
    let file = program
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let mut args = Vec::with_capacity(urls.len() + 1);
    if CHROMIUM.contains(&file.as_str()) {
        args.push("--new-window".to_owned());
    }
    args.extend(urls.iter().cloned());
    args
}

/// How Windows Terminal starts one pane.
#[derive(Clone, Debug, PartialEq)]
pub enum PaneLaunch {
    /// Bash in WSL with the rcfile at this `/tmp/runterm-*` path.
    Bash(String),
    /// PowerShell running this script, then staying interactive. `pwsh`
    /// selects PowerShell 7 instead of Windows PowerShell.
    Powershell { script: String, pwsh: bool },
    /// `ssh` in WSL, connecting to `host` and running this [`pane_script`].
    Ssh { host: String, script: String },
}

/// Windows limits a command line to 32,767 characters.
const MAX_COMMAND_LINE: usize = 32_000;

/// Focus moves "in order" rather than spatially, which is deterministic.
/// The pane name is only the initial title: programs such as Claude Code keep
/// their dynamic tab titles.
/// `profile` names the Windows Terminal profile whose appearance (colors,
/// font) the Bash panes use; PowerShell panes use the profile Windows Terminal
/// generates for their program. Windows Terminal falls back to its default
/// profile when no profile has that name. Without it, a command line gets the
/// bare `profiles.defaults` look.
pub fn terminal_args(
    layout: &Layout,
    distribution: &str,
    profile: &str,
    launches: &BTreeMap<String, PaneLaunch>,
) -> Result<Vec<String>, String> {
    tabs_args(&[TabLaunch {
        layout,
        distribution,
        profile,
        launches,
    }])
}

/// One Windows Terminal tab: a project's layout, with the arguments of
/// [`terminal_args`]. `launches` is per tab since projects sharing a template
/// share its pane ids.
#[derive(Clone, Copy, Debug)]
pub struct TabLaunch<'a> {
    pub layout: &'a Layout,
    pub distribution: &'a str,
    pub profile: &'a str,
    pub launches: &'a BTreeMap<String, PaneLaunch>,
}

/// Panes started by one launch, all tabs together.
pub const MAX_LAUNCH_PANES: usize = 64;

/// One new window with a tab per entry, the first one focused.
pub fn tabs_args(tabs: &[TabLaunch]) -> Result<Vec<String>, String> {
    if tabs.is_empty() {
        return Err("Aucun projet à lancer.".into());
    }
    if tabs.iter().map(|t| t.layout.panes().len()).sum::<usize>() > MAX_LAUNCH_PANES {
        return Err(format!(
            "Un lancement peut ouvrir au maximum {MAX_LAUNCH_PANES} panneaux."
        ));
    }
    let mut out = vec!["-w".into(), "new".into(), "--maximized".into()];
    for (index, tab) in tabs.iter().enumerate() {
        if index > 0 {
            out.push(";".into());
        }
        out.push("new-tab".into());
        tab_args(tab, &mut out)?;
    }
    if tabs.len() > 1 {
        out.extend([";", "focus-tab", "--target", "0"].map(String::from));
    }
    if out.iter().map(|a| a.len() + 3).sum::<usize>() > MAX_COMMAND_LINE {
        return Err(
            "Commande Windows Terminal trop longue : raccourcissez les actions PowerShell ou SSH."
                .into(),
        );
    }
    Ok(out)
}

/// Arguments following one `new-tab`.
fn tab_args(tab: &TabLaunch, out: &mut Vec<String>) -> Result<(), String> {
    let TabLaunch {
        layout,
        distribution,
        profile,
        launches,
    } = *tab;
    layout.validate(&mut HashSet::new(), 0)?;
    fn first_pane(node: &Layout) -> &Layout {
        match node {
            Layout::Pane { .. } => node,
            Layout::Split { first, .. } => first_pane(first),
        }
    }
    fn launch(
        pane: &Layout,
        distribution: &str,
        profile: &str,
        launches: &BTreeMap<String, PaneLaunch>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        let Layout::Pane { id, name, .. } = pane else {
            unreachable!()
        };
        match launches.get(id).ok_or("Script de panneau absent.")? {
            PaneLaunch::Bash(path) => {
                // Paths are generated by the adapter, never taken from project input.
                if !path.starts_with("/tmp/runterm-")
                    || !path
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"/.-_".contains(&c))
                {
                    return Err("Chemin de script invalide.".into());
                }
                if !profile.is_empty() {
                    out.extend(["--profile".into(), profile.into()]);
                }
                out.extend([
                    "--title".into(),
                    name.clone(),
                    "--".into(),
                    "wsl.exe".into(),
                ]);
                if !distribution.is_empty() {
                    out.extend(["--distribution".into(), distribution.into()]);
                }
                out.extend([
                    "--exec".into(),
                    "bash".into(),
                    "-lic".into(),
                    format!("exec bash --rcfile {} -i", shell_quote(path)),
                ]);
            }
            PaneLaunch::Ssh { host, script } => {
                if !profile.is_empty() {
                    out.extend(["--profile".into(), profile.into()]);
                }
                out.extend([
                    "--title".into(),
                    name.clone(),
                    "--".into(),
                    "wsl.exe".into(),
                ]);
                if !distribution.is_empty() {
                    out.extend(["--distribution".into(), distribution.into()]);
                }
                out.extend(["--exec".into(), "bash".into(), "-lic".into()]);
                out.push(ssh_command(host, script)?);
            }
            PaneLaunch::Powershell { script, pwsh } => {
                let (profile, program) = if *pwsh {
                    ("PowerShell", "pwsh.exe")
                } else {
                    ("Windows PowerShell", "powershell.exe")
                };
                out.extend(
                    [
                        "--profile",
                        profile,
                        "--title",
                        name,
                        "--",
                        program,
                        "-NoLogo",
                        "-NoExit",
                        "-EncodedCommand",
                    ]
                    .map(String::from),
                );
                out.push(encode_powershell(script));
            }
        }
        Ok(())
    }
    // `split-pane` splits whichever pane has focus, and focus can move on its
    // own while panes start (a starting control may grab it). Every split
    // therefore targets its pane by id: Windows Terminal numbers the panes of a
    // tab from 0 in creation order, which is the order `walk` creates them in.
    fn focus(index: usize, out: &mut Vec<String>) {
        out.extend([
            ";".into(),
            "focus-pane".into(),
            "--target".into(),
            index.to_string(),
        ]);
    }
    fn walk(
        node: &Layout,
        index: usize,
        next: &mut usize,
        dist: &str,
        profile: &str,
        launches: &BTreeMap<String, PaneLaunch>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Layout::Split {
            axis,
            ratio,
            first,
            second,
            ..
        } = node
        {
            focus(index, out);
            out.extend([
                ";".into(),
                "split-pane".into(),
                match axis {
                    Axis::Columns => "-V",
                    Axis::Rows => "-H",
                }
                .into(),
                "--size".into(),
                format!("{:.6}", 1.0 - ratio),
            ]);
            launch(first_pane(second), dist, profile, launches, out)?;
            let second_index = *next;
            *next += 1;
            walk(first, index, next, dist, profile, launches, out)?;
            walk(second, second_index, next, dist, profile, launches, out)?;
        }
        Ok(())
    }
    launch(first_pane(layout), distribution, profile, launches, out)?;
    walk(layout, 0, &mut 1, distribution, profile, launches, out)?;
    if matches!(layout, Layout::Split { .. }) {
        focus(0, out);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
