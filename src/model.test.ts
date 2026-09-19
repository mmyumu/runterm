import { describe, expect, it } from "vitest";
import {
  duplicateLayout,
  effectivePane,
  initialConfig,
  joinPath,
  moveItem,
  newPane,
  panes,
  removePane,
  updateNode,
  validateConfig,
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
