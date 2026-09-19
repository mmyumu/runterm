import { invoke, isTauri } from "@tauri-apps/api/core";
import { type Config, initialConfig, validateConfig } from "./model";
export const desktop = isTauri();
const key = "runterm-preview-v1";
export const api = {
  async load(): Promise<Config> {
    const data = desktop
      ? await invoke<Config | null>("load_config")
      : (JSON.parse(localStorage.getItem(key) ?? "null") as Config | null);
    if (!data) return initialConfig();
    const error = validateConfig(data);
    if (error) throw new Error(error);
    return data;
  },
  async save(config: Config): Promise<void> {
    const error = validateConfig(config);
    if (error) throw new Error(error);
    if (desktop) await invoke("save_config", { config });
    else localStorage.setItem(key, JSON.stringify(config));
  },
  distributions: () =>
    desktop
      ? invoke<string[]>("list_distributions")
      : Promise.resolve<string[]>([]),
  async launch(config: Config, projectId: string): Promise<void> {
    if (!desktop)
      throw new Error(
        "Le lancement est disponible dans l’application Windows. Cette page est un aperçu de l’éditeur.",
      );
    await invoke("launch_project", { config, projectId });
  },
};
