import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { api } from "./api";
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
beforeEach(() => localStorage.clear());
it("crée et sauvegarde un projet avec une commande personnalisée", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  const name = screen.getByLabelText("Nom du projet");
  await user.clear(name);
  await user.type(name, "Betalab");
  const root = screen.getByLabelText("Dossier racine WSL");
  await user.clear(root);
  await user.type(root, "/home/me/workspaces/betalab");
  await user.click(
    screen.getByRole("button", { name: "Sélectionner le panneau Backend" }),
  );
  const command = screen.getByLabelText("Commande 1");
  await user.clear(command);
  await user.type(command, "uv run uvicorn app.main:app --reload");
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("enregistrées"),
  );
  const saved = JSON.parse(localStorage.getItem("runterm-preview-v1")!);
  expect(saved.projects[0].root).toBe("/home/me/workspaces/betalab");
  expect(Object.values(saved.projects[0].overrides)).toEqual([
    { commands: ["uv run uvicorn app.main:app --reload"] },
  ]);
  expect(
    (
      screen.getByRole("button", {
        name: "Lancer le projet",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  cleanup();
  render(<App />);
  expect(await screen.findByDisplayValue("Betalab")).toBeTruthy();
});
it("modifie un modèle visuellement et protège un modèle utilisé", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  await user.click(screen.getByRole("button", { name: "Modifier le modèle" }));
  await user.click(screen.getByRole("button", { name: "Gauche / droite" }));
  expect(
    screen.getAllByRole("button", { name: /Sélectionner le panneau/ }),
  ).toHaveLength(6);
  await user.click(
    screen.getByRole("button", { name: "Supprimer le panneau" }),
  );
  expect(
    screen.getAllByRole("button", { name: /Sélectionner le panneau/ }),
  ).toHaveLength(5);
  await user.click(screen.getByRole("button", { name: "Supprimer" }));
  const dialog = screen.getByRole("dialog");
  await user.click(
    Array.from(dialog.querySelectorAll("button")).find(
      (b) => b.textContent === "Supprimer",
    )!,
  );
  expect(screen.getByRole("alert").textContent).toContain("utilisé");
});
it("passe un panneau en PowerShell dans le modèle", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  await user.click(screen.getByRole("button", { name: "Modifier le modèle" }));
  await user.click(
    screen.getByRole("button", { name: "Sélectionner le panneau Frontend" }),
  );
  await user.selectOptions(screen.getByLabelText("Shell"), "powershell");
  expect(screen.getByText("BASH / WSL + POWERSHELL")).toBeTruthy();
  expect(screen.getByText("PS")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Ajouter une action" }));
  expect(
    screen.getByPlaceholderText("Votre commande PowerShell…"),
  ).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("enregistrées"),
  );
  const saved = JSON.parse(localStorage.getItem("runterm-preview-v1")!);
  const shells = JSON.stringify(saved.templates[0].layout).match(
    /"shell":"\w+"/g,
  );
  expect(shells).toEqual(['"shell":"powershell"']);
  await user.selectOptions(screen.getByLabelText("Shell"), "bash");
  expect(screen.getByText("BASH / WSL")).toBeTruthy();
});
it("ne remplace pas une configuration illisible", async () => {
  localStorage.setItem("runterm-preview-v1", "broken");
  render(<App />);
  expect(
    await screen.findByRole("heading", { name: "Configuration inaccessible" }),
  ).toBeTruthy();
  expect(localStorage.getItem("runterm-preview-v1")).toBe("broken");
});
it("réordonne les projets au clavier", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  await user.click(screen.getByRole("button", { name: "Créer un projet" }));
  const name = screen.getByLabelText("Nom du projet");
  await user.clear(name);
  await user.type(name, "Second");
  const entries = () =>
    Array.from(document.querySelectorAll(".sidebar-entry")).map(
      (e) => e.textContent,
    );
  expect(entries()).toEqual(["Nouveau projet", "Second"]);
  screen.getByRole("button", { name: "Second" }).focus();
  await user.keyboard("{Alt>}{ArrowUp}{/Alt}");
  expect(entries()).toEqual(["Second", "Nouveau projet"]);
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("enregistrées"),
  );
  const saved = JSON.parse(localStorage.getItem("runterm-preview-v1")!);
  expect(saved.projects.map((p: { name: string }) => p.name)).toEqual([
    "Second",
    "Nouveau projet",
  ]);
});
it("propose la mise à jour disponible et l’installe", async () => {
  const install = vi.fn(async (onProgress: (p: number | null) => void) => {
    onProgress(42);
    await new Promise(() => {});
  });
  vi.spyOn(api, "checkUpdate").mockResolvedValue({
    version: "0.2.0",
    currentVersion: "0.1.0",
    install,
  });
  const user = userEvent.setup();
  render(<App />);
  expect(await screen.findByText("RunTerm 0.2.0 est disponible")).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Installer et redémarrer" }),
  );
  expect(install).toHaveBeenCalledOnce();
  expect(await screen.findByText("Téléchargement… 42 %")).toBeTruthy();
});
it("bloque l’installation tant que des modifications ne sont pas enregistrées", async () => {
  const install = vi.fn();
  vi.spyOn(api, "checkUpdate").mockResolvedValue({
    version: "0.2.0",
    currentVersion: "0.1.0",
    install,
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  const button = screen.getByRole("button", {
    name: "Installer et redémarrer",
  }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  expect(
    screen.getByText(/Enregistrez vos modifications avant de l’installer/),
  ).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() => expect(button.disabled).toBe(false));
});
it("ignore une mise à jour proposée", async () => {
  vi.spyOn(api, "checkUpdate").mockResolvedValue({
    version: "0.2.0",
    currentVersion: "0.1.0",
    install: vi.fn(),
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Ignorer la mise à jour" }),
  );
  expect(screen.queryByText("RunTerm 0.2.0 est disponible")).toBeNull();
});
it("préremplit la racine depuis le workspace et propose ses sous-dossiers", async () => {
  const directories = vi
    .spyOn(api, "directories")
    .mockResolvedValue(["alpha", "beta"]);
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Paramètres" }));
  await user.type(
    screen.getByLabelText("Racine des workspaces WSL"),
    "/home/me/ws",
  );
  await user.click(screen.getByRole("button", { name: "Fermer" }));
  await user.click(
    screen.getByRole("button", { name: "Créer mon premier projet" }),
  );
  const root = screen.getByLabelText("Dossier racine WSL") as HTMLInputElement;
  expect(root.value).toBe("/home/me/ws/");
  await waitFor(() =>
    expect(
      Array.from(document.querySelectorAll("#workspace-folders option")).map(
        (o) => (o as HTMLOptionElement).value,
      ),
    ).toEqual(["/home/me/ws/alpha", "/home/me/ws/beta"]),
  );
  expect(directories).toHaveBeenLastCalledWith("", "/home/me/ws");
  await user.type(root, "beta");
  expect(
    (screen.getByLabelText("Nom du projet") as HTMLInputElement).value,
  ).toBe("beta");
  await user.clear(root);
  await user.type(root, "/opt/manual");
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("enregistrées"),
  );
  const saved = JSON.parse(localStorage.getItem("runterm-preview-v1")!);
  expect(saved.workspaceRoot).toBe("/home/me/ws");
  expect(saved.projects[0]).toMatchObject({
    name: "beta",
    root: "/opt/manual",
  });
});
it("choisit les projets lancés par « Tout lancer » et s’en souvient", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Créer mon premier projet" }),
  );
  await user.click(screen.getByRole("button", { name: "Créer un projet" }));
  const name = screen.getByLabelText("Nom du projet");
  await user.clear(name);
  await user.type(name, "Second");
  const group = screen.getByRole("group", { name: "Tout lancer" });
  expect(group.textContent).toContain("Aucun projet activé");
  const toggle = screen.getByRole("button", {
    name: "Inclure Second dans « Tout lancer »",
  });
  expect(toggle.getAttribute("aria-pressed")).toBe("false");
  await user.click(toggle);
  expect(toggle.getAttribute("aria-pressed")).toBe("true");
  expect(group.textContent).toContain("1 projet");
  // Toggling does not open the project in the editor.
  expect(screen.getByLabelText<HTMLInputElement>("Nom du projet").value).toBe(
    "Second",
  );
  await user.click(
    screen.getByRole("button", {
      name: "Inclure Nouveau projet dans « Tout lancer »",
    }),
  );
  expect(group.textContent).toContain("2 projets");
  // Launching needs the Windows app.
  for (const label of ["Tout lancer en onglets", "Tout lancer en fenêtres"])
    expect(
      screen.getByRole<HTMLButtonElement>("button", { name: label }).disabled,
    ).toBe(true);
  await user.click(
    screen.getByRole("button", {
      name: "Inclure Nouveau projet dans « Tout lancer »",
    }),
  );
  await user.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(screen.getByRole("status").textContent).toContain("enregistrées"),
  );
  const saved = JSON.parse(localStorage.getItem("runterm-preview-v1")!);
  expect(
    saved.projects.map((p: { name: string; launchAll?: boolean }) => [
      p.name,
      p.launchAll,
    ]),
  ).toEqual([
    ["Nouveau projet", undefined],
    ["Second", true],
  ]);
  cleanup();
  render(<App />);
  expect(
    (
      await screen.findByRole("button", {
        name: "Inclure Second dans « Tout lancer »",
      })
    ).getAttribute("aria-pressed"),
  ).toBe("true");
});
