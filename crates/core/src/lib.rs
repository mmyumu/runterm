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
    pub distribution: String,
    /// Windows Terminal profile (name or GUID) giving the panes their look.
    /// Empty: the profile named after the distribution.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub terminal_profile: String,
    pub template_id: String,
    pub overrides: BTreeMap<String, Override>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    pub templates: Vec<Template>,
    pub projects: Vec<Project>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPane {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub commands: Vec<String>,
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

/// Focus moves "in order" rather than spatially, which is deterministic.
/// The pane name is only the initial title: programs such as Claude Code keep
/// their dynamic tab titles.
/// `profile` names the Windows Terminal profile whose appearance (colors,
/// font) the panes use; Windows Terminal falls back to its default profile
/// when no profile has that name. Without it, a command line gets the bare
/// `profiles.defaults` look.
pub fn terminal_args(
    layout: &Layout,
    distribution: &str,
    profile: &str,
    scripts: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
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
        scripts: &BTreeMap<String, String>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        let Layout::Pane { id, name, .. } = pane else {
            unreachable!()
        };
        let path = scripts.get(id).ok_or("Script de panneau absent.")?;
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
        Ok(())
    }
    // Windows Terminal moves focus "in order" along the leaves of its pane
    // tree, where a new pane sits right after the pane it was split from.
    // `order` mirrors that sequence using creation indices.
    fn focus(index: usize, current: &mut usize, order: &[usize], out: &mut Vec<String>) {
        let position = |i| order.iter().position(|&p| p == i).unwrap();
        let (from, to) = (position(*current), position(index));
        let direction = if from < to {
            "nextInOrder"
        } else {
            "previousInOrder"
        };
        for _ in 0..from.abs_diff(to) {
            out.extend([";".into(), "move-focus".into(), direction.into()]);
        }
        *current = index;
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        node: &Layout,
        index: usize,
        next: &mut usize,
        current: &mut usize,
        order: &mut Vec<usize>,
        dist: &str,
        profile: &str,
        scripts: &BTreeMap<String, String>,
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
            focus(index, current, order, out);
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
            launch(first_pane(second), dist, profile, scripts, out)?;
            let second_index = *next;
            *next += 1;
            let at = order.iter().position(|&p| p == index).unwrap();
            order.insert(at + 1, second_index);
            *current = second_index;
            walk(
                first, index, next, current, order, dist, profile, scripts, out,
            )?;
            walk(
                second,
                second_index,
                next,
                current,
                order,
                dist,
                profile,
                scripts,
                out,
            )?;
        }
        Ok(())
    }
    let mut out = vec![
        "-w".into(),
        "new".into(),
        "--maximized".into(),
        "new-tab".into(),
    ];
    launch(first_pane(layout), distribution, profile, scripts, &mut out)?;
    let (mut current, mut order) = (0, vec![0]);
    walk(
        layout,
        0,
        &mut 1,
        &mut current,
        &mut order,
        distribution,
        profile,
        scripts,
        &mut out,
    )?;
    focus(0, &mut current, &order, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests;
