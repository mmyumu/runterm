import { invoke, isTauri } from "@tauri-apps/api/core";
import { type Config, initialConfig, validateConfig } from "./model";
export const desktop = isTauri();
const key = "runterm-preview-v1";
export type AvailableUpdate = {
  version: string;
  currentVersion: string;
  /** Downloads, then runs the installer; on Windows the app exits and the installer restarts it. */
  install(onProgress: (percent: number | null) => void): Promise<void>;
};
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
  /** Visible subfolders of a WSL folder; always empty in the browser preview. */
  directories: (distribution: string, path: string) =>
    desktop
      ? invoke<string[]>("list_directories", { distribution, path })
      : Promise.resolve<string[]>([]),
  async launch(config: Config, projectId: string): Promise<void> {
    if (!desktop)
      throw new Error(
        "Le lancement est disponible dans l’application Windows. Cette page est un aperçu de l’éditeur.",
      );
    await invoke("launch_project", { config, projectId });
  },
  version: () =>
    desktop
      ? import("@tauri-apps/api/app").then(({ getVersion }) => getVersion())
      : Promise.resolve(null),
  async checkUpdate(): Promise<AvailableUpdate | null> {
    if (!desktop) return null;
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) return null;
    return {
      version: update.version,
      currentVersion: update.currentVersion,
      async install(onProgress) {
        let total = 0;
        let received = 0;
        await update.downloadAndInstall((event) => {
          if (event.event === "Started") total = event.data.contentLength ?? 0;
          else if (event.event === "Progress") {
            received += event.data.chunkLength;
            onProgress(total ? Math.min(100, (received / total) * 100) : null);
          }
        });
      },
    };
  },
};
