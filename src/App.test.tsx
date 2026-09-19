import { afterEach, beforeEach, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
afterEach(cleanup);
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
