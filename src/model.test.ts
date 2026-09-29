import { describe, expect, it } from "vitest";
import {
  duplicateLayout,
  effectivePane,
  initialConfig,
  joinPath,
  launchWindows,
  migrateConfig,
  moveItem,
  newPane,
  panes,
  removePane,
  updateNode,
  validateConfig,
  type Config,
  type Layout,
  type Project,
} from "./model";
describe("layouts partagés", () => {
  it("conserve les identifiants pendant une modification et recrée ceux des copies", () => {
    const original = initialConfig().templates[0].layout;
    const first = panes(original)[0];
    const edited = updateNode(original, first.id, (node) => ({
      ...node,
      name: "Assistant",
    }));
    expect(panes(edited)[0].id).toBe(first.id);
    expect(panes(edited)[0].name).toBe("Assistant");
    const copy = duplicateLayout(edited);
    expect(
      panes(copy)
        .map((p) => p.id)
        .some((id) => panes(edited).some((p) => p.id === id)),
    ).toBe(false);
    expect(panes(copy).map((p) => p.name)).toEqual(
      panes(edited).map((p) => p.name),
    );
  });
  it("agrandit le voisin lorsqu’un panneau est supprimé", () => {
    const first = newPane("A");
    const second = newPane("B");
    const layout: Layout = {
      kind: "split",
      id: "split",
      axis: "columns",
      ratio: 0.3,
      first,
      second,
    };
    expect(removePane(layout, first.id)).toEqual(second);
  });
  it("hérite des changements sauf pour les champs personnalisés", () => {
    const pane = {
      ...newPane("Backend"),
      commands: ["server"],
      directory: "backend",
    };
    const project: Project = {
      id: "p",
      name: "P",
      distribution: "",
      root: "/p",
      templateId: "t",
      overrides: { [pane.id]: { commands: [] } },
    };
    expect(
      effectivePane(
        { ...pane, directory: "api", commands: ["new server"] },
        project,
      ),
    ).toMatchObject({ directory: "api", commands: [] });
    expect(effectivePane(pane, { ...project, overrides: {} }).commands).toEqual(
      ["server"],
    );
  });
  it("valide le profil Windows Terminal optionnel", () => {
    const config = initialConfig();
    const project: Project = {
      id: "p",
      name: "P",
      distribution: "",
      root: "/p",
      templateId: config.templates[0].id,
      overrides: {},
    };
    const withProfile = (terminalProfile?: string) =>
      validateConfig({
        ...config,
        projects: [{ ...project, terminalProfile }],
      });
    expect(withProfile(undefined)).toBeNull();
    expect(withProfile("Ubuntu 24.04.1 LTS")).toBeNull();
    expect(withProfile("a;b")).not.toBeNull();
    expect(withProfile("--help")).not.toBeNull();
  });
  it("valide l’hôte SSH optionnel", () => {
    const config = initialConfig();
    const withHost = (host?: string) =>
      validateConfig({
        ...config,
        projects: [
          {
            id: "p",
            name: "P",
            distribution: "",
            root: "/p",
            host,
            templateId: config.templates[0].id,
            overrides: {},
          },
        ],
      });
    expect(withHost(undefined)).toBeNull();
    expect(withHost("me@dev-vm.example.com")).toBeNull();
    expect(withHost("-oProxyCommand=x")).not.toBeNull();
    expect(withHost("a b")).not.toBeNull();
    expect(withHost("a;b")).not.toBeNull();
  });
  it("valide l’URL optionnelle du projet", () => {
    const config = initialConfig();
    const withUrl = (url?: string, urlDisabled?: boolean) =>
      validateConfig({
        ...config,
        projects: [
          {
            id: "p",
            name: "P",
            distribution: "",
            root: "/p",
            url,
            urlDisabled,
            templateId: config.templates[0].id,
            overrides: {},
          },
        ],
      });
    expect(withUrl(undefined)).toBeNull();
    expect(withUrl("http://localhost:5173")).toBeNull();
    expect(withUrl("https://app.example.com/d?tab=1#top")).toBeNull();
    expect(withUrl("localhost:5173")).not.toBeNull();
    expect(withUrl("file:///C:/page.html")).not.toBeNull();
    expect(withUrl("http://")).not.toBeNull();
    expect(withUrl("http://a b")).not.toBeNull();
    // Décochée, l’adresse est conservée et tenue aux mêmes règles.
    expect(withUrl("http://localhost:5173", true)).toBeNull();
    expect(withUrl("http://a b", true)).not.toBeNull();
  });
  it("valide le dossier VS Code optionnel", () => {
    const config = initialConfig();
    const withFolder = (vscodeFolder?: string) =>
      validateConfig({
        ...config,
        projects: [
          {
            id: "p",
            name: "P",
            distribution: "",
            root: "/p",
            vscode: true,
            vscodeFolder,
            templateId: config.templates[0].id,
            overrides: {},
          },
        ],
      });
    expect(withFolder(undefined)).toBeNull();
    expect(withFolder("frontend")).toBeNull();
    expect(withFolder("/srv/autre projet")).toBeNull();
    expect(withFolder("a\nb")).not.toBeNull();
  });
  it("valide la racine des workspaces optionnelle", () => {
    const config = initialConfig();
    expect(validateConfig({ ...config, workspaceRoot: "" })).toBeNull();
    expect(
      validateConfig({ ...config, workspaceRoot: "/home/me/workspaces" }),
    ).toBeNull();
    expect(
      validateConfig({ ...config, workspaceRoot: "workspaces" }),
    ).not.toBeNull();
    expect(joinPath("/home/me/ws", "a")).toBe("/home/me/ws/a");
    expect(joinPath("/", "a")).toBe("/a");
  });
});
describe("ordre des éléments", () => {
  it("déplace un élément à la place d’un autre", () => {
    const items = ["a", "b", "c", "d"].map((id) => ({ id }));
    const ids = (list: { id: string }[]) => list.map((item) => item.id);
    expect(ids(moveItem(items, "a", "c"))).toEqual(["b", "c", "a", "d"]);
    expect(ids(moveItem(items, "d", "b"))).toEqual(["a", "d", "b", "c"]);
    expect(moveItem(items, "a", "a")).toBe(items);
    expect(moveItem(items, "x", "a")).toBe(items);
  });
});
describe("groupes de lancement", () => {
  const withProjects = () => {
    const config = initialConfig();
    const project = (id: string, launchAll?: boolean): Project => ({
      id,
      name: id,
      root: "/home/me",
      distribution: "",
      templateId: config.templates[0].id,
      overrides: {},
      ...(launchAll === undefined ? {} : { launchAll }),
    });
    return { ...config, projects: [project("a", true), project("b", false)] };
  };
  it("reprend l’ancienne sélection « Tout lancer » dans un groupe", () => {
    const migrated = migrateConfig(withProjects());
    expect(migrated.launchGroups).toEqual([
      { id: expect.any(String), name: "Principal", windows: { a: 1 } },
    ]);
    expect(migrated.projects.some((p) => "launchAll" in p)).toBe(false);
    expect(migrateConfig(migrated)).toBe(migrated);
  });
  it("place en fenêtre 1 la sélection des premiers groupes", () => {
    const config = {
      ...withProjects(),
      launchGroups: [{ id: "g", name: "Perso", projectIds: ["a", "b"] }],
    } as unknown as Config;
    const migrated = migrateConfig(config);
    expect(migrated.launchGroups).toEqual([
      { id: "g", name: "Perso", windows: { a: 1, b: 1 } },
    ]);
    expect(validateConfig(migrated)).toBeNull();
  });
  it("refuse un groupe sans nom ou vers un projet absent", () => {
    const config = migrateConfig(withProjects());
    const group = config.launchGroups![0];
    expect(validateConfig(config)).toBeNull();
    expect(
      validateConfig({ ...config, launchGroups: [{ ...group, name: " " }] }),
    ).toContain("nom au groupe");
    expect(
      validateConfig({
        ...config,
        launchGroups: [{ ...group, windows: { x: 1 } }],
      }),
    ).toContain("projet absent");
    for (const window of [0, 5, 1.5])
      expect(
        validateConfig({
          ...config,
          launchGroups: [{ ...group, windows: { a: window } }],
        }),
      ).toContain("de 1 à 4");
  });
  it("ouvre les fenêtres par numéro, les onglets dans l’ordre de la liste", () => {
    const config = withProjects();
    const group = { id: "g", name: "Boulot", windows: { b: 3, a: 3 } };
    expect(launchWindows(config, group)).toEqual([["a", "b"]]);
    expect(
      launchWindows(config, { ...group, windows: { b: 1, a: 4 } }),
    ).toEqual([["b"], ["a"]]);
    expect(launchWindows(config, { ...group, windows: {} })).toEqual([]);
    expect(launchWindows(config)).toEqual([]);
  });
});
