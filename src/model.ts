export type Pane = {
  kind: "pane";
  id: string;
  name: string;
  directory: string;
  commands: string[];
  /** Absent: Bash in WSL. PowerShell runs on Windows in the same folder. */
  shell?: Shell;
};
export type Shell = "bash" | "powershell";
export type Split = {
  kind: "split";
  id: string;
  axis: "columns" | "rows";
  ratio: number;
  first: Layout;
  second: Layout;
};
export type Layout = Pane | Split;
export type Template = { id: string; name: string; layout: Layout };
export type PaneOverride = {
  directory?: string | null;
  commands?: string[] | null;
};
export type Project = {
  id: string;
  name: string;
  root: string;
  /** SSH destination reached with the distribution's `ssh`; `root` is then on that host. Absent: WSL. */
  host?: string;
  distribution: string;
  /** Windows Terminal profile (name or GUID); absent: named after the distribution. */
  terminalProfile?: string;
  /** Page opened in the default browser at launch (http(s)); absent: none. */
  url?: string;
  /** Keeps `url` without opening it at launch; absent: the URL opens. */
  urlDisabled?: boolean;
  /** Opens the project in VS Code at launch, remotely in WSL (or on `host` over SSH); absent: no. */
  vscode?: boolean;
  /** Folder VS Code opens: relative to `root`, or absolute; absent: `root`. */
  vscodeFolder?: string;
  templateId: string;
  overrides: Record<string, PaneOverride>;
  /** Started by the "launch all" buttons; absent: not started. */
  launchAll?: boolean;
};
export type Config = {
  schemaVersion: number;
  /** WSL folder holding the projects: prefills new roots and lists their subfolders. */
  workspaceRoot?: string;
  templates: Template[];
  projects: Project[];
};
export const uid = () => crypto.randomUUID();
export const newPane = (name = "Terminal"): Pane => ({
  kind: "pane",
  id: uid(),
  name,
  directory: ".",
  commands: [],
});
export function panes(node: Layout): Pane[] {
  return node.kind === "pane"
    ? [node]
    : [...panes(node.first), ...panes(node.second)];
}
export function updateNode(
  node: Layout,
  id: string,
  update: (node: Layout) => Layout,
): Layout {
  if (node.id === id) return update(node);
  return node.kind === "pane"
    ? node
    : {
        ...node,
        first: updateNode(node.first, id, update),
        second: updateNode(node.second, id, update),
      };
}
export function removePane(node: Layout, id: string): Layout {
  if (node.kind === "pane") return node;
  if (node.first.id === id) return node.second;
  if (node.second.id === id) return node.first;
  return {
    ...node,
    first: removePane(node.first, id),
    second: removePane(node.second, id),
  };
}
export function duplicateLayout(node: Layout): Layout {
  return node.kind === "pane"
    ? { ...node, id: uid(), commands: [...node.commands] }
    : {
        ...node,
        id: uid(),
        first: duplicateLayout(node.first),
        second: duplicateLayout(node.second),
      };
}
export function moveItem<T extends { id: string }>(
  items: T[],
  id: string,
  targetId: string,
): T[] {
  const from = items.findIndex((item) => item.id === id);
  const to = items.findIndex((item) => item.id === targetId);
  if (from < 0 || to < 0 || from === to) return items;
  const next = [...items];
  next.splice(to, 0, ...next.splice(from, 1));
  return next;
}
export const joinPath = (root: string, name: string) =>
  root.endsWith("/") ? `${root}${name}` : `${root}/${name}`;
export function effectivePane(pane: Pane, project?: Project): Pane {
  const custom = project?.overrides[pane.id];
  return {
    ...pane,
    directory: custom?.directory ?? pane.directory,
    commands: custom?.commands ?? pane.commands,
  };
}
export function initialConfig(): Config {
  const codex = { ...newPane("Codex"), commands: ["codex"] };
  const shell = newPane("Terminal");
  const backend = {
    ...newPane("Backend"),
    directory: "backend",
    commands: ["uv run uvicorn app.main:app --reload"],
  };
  const frontend = { ...newPane("Frontend"), directory: "frontend" };
  const claude = { ...newPane("Claude"), commands: ["claude"] };
  const split = (
    axis: Split["axis"],
    first: Layout,
    second: Layout,
  ): Split => ({ kind: "split", id: uid(), axis, ratio: 0.5, first, second });
  return {
    schemaVersion: 1,
    projects: [],
    templates: [
      {
        id: uid(),
        name: "Workspace développeur",
        layout: split(
          "rows",
          split("columns", codex, claude),
          split("columns", shell, split("columns", backend, frontend)),
        ),
      },
    ],
  };
}
export function validateConfig(config: Config): string | null {
  if (
    config.schemaVersion !== 1 ||
    !Array.isArray(config.templates) ||
    !Array.isArray(config.projects)
  )
    return "Configuration non prise en charge.";
  const workspaceRoot = config.workspaceRoot ?? "";
  if (workspaceRoot && !workspaceRoot.startsWith("/"))
    return "La racine des workspaces doit être un chemin Linux absolu.";
  for (const template of config.templates) {
    if (!template.name.trim()) return "Donnez un nom au modèle.";
    if (panes(template.layout).length > 16)
      return "Un layout peut contenir au maximum 16 panneaux.";
    for (const pane of panes(template.layout))
      if (!pane.name.trim()) return "Donnez un nom à chaque panneau.";
  }
  for (const project of config.projects) {
    if (!project.name.trim()) return "Donnez un nom au projet.";
    if (!project.root.startsWith("/"))
      return "La racine du projet doit être un chemin Linux absolu.";
    const host = project.host ?? "";
    if (host.startsWith("-") || !/^[A-Za-z0-9._@:-]*$/.test(host))
      return "Hôte SSH invalide (lettres, chiffres et « . _ - @ : » uniquement).";
    const profile = project.terminalProfile ?? "";
    if (/[\0\r\n;]/.test(profile) || profile.trimStart().startsWith("-"))
      return "Profil Windows Terminal invalide (point-virgule, retour à la ligne ou tiret initial).";
    const url = project.url ?? "";
    if (
      url &&
      (url.length > 2000 || !/^https?:\/\/[^\s"'<>|\p{C}]+$/u.test(url))
    )
      return "L’URL du projet doit commencer par http:// ou https://, sans espace.";
    if (/\p{Cc}/u.test(project.vscodeFolder ?? ""))
      return "Dossier VS Code invalide (caractère de contrôle).";
    if (!config.templates.some((t) => t.id === project.templateId))
      return "Choisissez un modèle existant.";
  }
  return null;
}
