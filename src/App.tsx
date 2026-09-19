import { useEffect, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronRight,
  Columns2,
  Copy,
  Download,
  Folder,
  FolderOpen,
  Layers,
  LayoutTemplate,
  LoaderCircle,
  PanelLeftClose,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  Rows2,
  Save,
  Terminal,
  Trash2,
  X,
} from "lucide-react";
import { type AvailableUpdate, api, desktop } from "./api";
import {
  type Config,
  type Layout,
  type Pane,
  type Project,
  type Template,
  duplicateLayout,
  effectivePane,
  moveItem,
  newPane,
  panes,
  removePane,
  uid,
  updateNode,
  validateConfig,
} from "./model";

function LayoutPreview({
  node,
  selected,
  onSelect,
  onRatio,
  project,
  compact = false,
}: {
  node: Layout;
  selected?: string;
  onSelect?: (id: string) => void;
  onRatio?: (id: string, ratio: number) => void;
  project?: Project;
  compact?: boolean;
}) {
  if (node.kind === "pane") {
    const pane = effectivePane(node, project);
    return (
      <button
        type="button"
        className={`terminal-pane ${selected === node.id ? "selected" : ""} ${compact ? "compact" : ""}`}
        onClick={() => onSelect?.(node.id)}
        aria-label={`Sélectionner le panneau ${pane.name}`}
      >
        <span className="pane-bar">
          <span className="pane-dot" />
          <span>{pane.name}</span>
          {!compact && <Terminal size={13} />}
        </span>
        {!compact && (
          <span className="pane-content">
            <span className="pane-directory">
              ~/ {pane.directory === "." ? "projet" : pane.directory}
            </span>
            <span className="pane-command">
              <span className="prompt">❯</span>{" "}
              {pane.commands[0] || (
                <span className="muted">shell interactif</span>
              )}
            </span>
            {pane.commands.length > 1 && (
              <span className="more-commands">
                + {pane.commands.length - 1} action
                {pane.commands.length > 2 ? "s" : ""}
              </span>
            )}
            <span className="cursor" />
          </span>
        )}
      </button>
    );
  }
  const columns = node.axis === "columns";
  const startDrag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!onRatio) return;
    const rect = event.currentTarget.parentElement!.getBoundingClientRect();
    const move = (e: PointerEvent) => {
      const ratio = columns
        ? (e.clientX - rect.left) / rect.width
        : (e.clientY - rect.top) / rect.height;
      onRatio(
        node.id,
        Math.round(Math.max(0.1, Math.min(0.9, ratio)) * 100) / 100,
      );
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
    event.preventDefault();
  };
  return (
    <div className={`layout-split ${columns ? "columns" : "rows"}`}>
      <div className="split-child" style={{ flex: `${node.ratio} 1 0` }}>
        <LayoutPreview
          {...{ selected, onSelect, onRatio, project, compact }}
          node={node.first}
        />
      </div>
      <div
        className={`separator ${onRatio ? "adjustable" : ""}`}
        role="separator"
        aria-label="Proportion des panneaux"
        aria-orientation={columns ? "vertical" : "horizontal"}
        aria-valuemin={10}
        aria-valuemax={90}
        aria-valuenow={Math.round(node.ratio * 100)}
        tabIndex={onRatio ? 0 : undefined}
        onPointerDown={startDrag}
        onKeyDown={(e) => {
          if (
            onRatio &&
            ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)
          ) {
            e.preventDefault();
            onRatio(
              node.id,
              Math.max(
                0.1,
                Math.min(
                  0.9,
                  node.ratio +
                    (["ArrowLeft", "ArrowUp"].includes(e.key) ? -0.05 : 0.05),
                ),
              ),
            );
          }
        }}
      />
      <div className="split-child" style={{ flex: `${1 - node.ratio} 1 0` }}>
        <LayoutPreview
          {...{ selected, onSelect, onRatio, project, compact }}
          node={node.second}
        />
      </div>
    </div>
  );
}

export default function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [baseline, setBaseline] = useState("");
  const [view, setView] = useState<"projects" | "templates">("projects");
  const [selectedId, setSelectedId] = useState("");
  const [paneId, setPaneId] = useState("");
  const [distributions, setDistributions] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [loadError, setLoadError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState<"save" | "launch" | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [dragId, setDragId] = useState("");
  const [version, setVersion] = useState<string | null>(null);
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  // undefined: not installing; null: downloading with unknown size.
  const [updateProgress, setUpdateProgress] = useState<number | null>();
  const alive = useRef(true);
  const message = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // The action buttons sit at the bottom of long forms: bring feedback into view.
    message.current?.scrollIntoView?.({ block: "nearest", behavior: "smooth" });
  }, [error, notice]);
  useEffect(() => {
    alive.current = true;
    api
      .load()
      .then((data) => {
        if (alive.current) {
          setConfig(data);
          setBaseline(JSON.stringify(data));
          setSelectedId(data.projects[0]?.id ?? "");
        }
      })
      .catch((e) => {
        if (alive.current) setLoadError(String(e));
      });
    api
      .distributions()
      .then((data) => {
        if (alive.current) setDistributions(data);
      })
      .catch((e) => {
        if (alive.current) setError(String(e));
      });
    api
      .version()
      .then((data) => {
        if (alive.current) setVersion(data);
      })
      .catch(() => {});
    // Silent at startup: being offline must not show an error.
    api
      .checkUpdate()
      .then((data) => {
        if (alive.current) setUpdate(data);
      })
      .catch(() => {});
    return () => {
      alive.current = false;
    };
  }, []);
  const dirty = config !== null && JSON.stringify(config) !== baseline;
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty]);
  useEffect(() => {
    if (!desktop || !dirty) return;
    let cleanup: (() => void) | undefined;
    let disposed = false;
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) =>
        getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault();
          setError(
            "Enregistrez vos modifications avant de fermer RunTerm, ou utilisez « Annuler les modifications ».",
          );
        }),
      )
      .then((unlisten) => {
        if (disposed) unlisten();
        else cleanup = unlisten;
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [dirty]);
  if (loadError)
    return (
      <main className="fatal">
        <Terminal size={36} />
        <h1>Configuration inaccessible</h1>
        <p>{loadError}</p>
        <p>
          Votre fichier a été conservé. Corrigez-le puis relancez l’application.
        </p>
        <button onClick={() => location.reload()}>Réessayer</button>
      </main>
    );
  if (!config)
    return (
      <main className="fatal">
        <LoaderCircle className="spin" />
        <p>Ouverture de RunTerm…</p>
      </main>
    );
  const project =
    view === "projects"
      ? config.projects.find((p) => p.id === selectedId)
      : undefined;
  const template = config.templates.find(
    (t) => t.id === (view === "templates" ? selectedId : project?.templateId),
  );
  const allPanes = template ? panes(template.layout) : [];
  const selectedPane = allPanes.find((p) => p.id === paneId) ?? allPanes[0];
  const effective = selectedPane
    ? effectivePane(selectedPane, project)
    : undefined;
  const mutate = (fn: (data: Config) => Config) => {
    setConfig((data) => data && fn(data));
    setNotice("");
    setError("");
  };
  const editProject = (patch: Partial<Project>) =>
    mutate((data) => ({
      ...data,
      projects: data.projects.map((p) =>
        p.id === project?.id ? { ...p, ...patch } : p,
      ),
    }));
  const editTemplate = (patch: Partial<Template>) =>
    mutate((data) => ({
      ...data,
      templates: data.templates.map((t) =>
        t.id === template?.id ? { ...t, ...patch } : t,
      ),
    }));
  const changeLayout = (fn: (layout: Layout) => Layout) =>
    mutate((data) => ({
      ...data,
      templates: data.templates.map((t) =>
        t.id === template?.id ? { ...t, layout: fn(t.layout) } : t,
      ),
    }));
  const editPane = (patch: Partial<Pane>) => {
    if (!selectedPane) return;
    if (project) {
      const existing = project.overrides[selectedPane.id] ?? {};
      editProject({
        overrides: {
          ...project.overrides,
          [selectedPane.id]: {
            ...existing,
            ...("directory" in patch ? { directory: patch.directory } : {}),
            ...("commands" in patch ? { commands: patch.commands } : {}),
          },
        },
      });
    } else
      changeLayout((layout) =>
        updateNode(
          layout,
          selectedPane.id,
          (node) => ({ ...node, ...patch }) as Pane,
        ),
      );
  };
  const chooseView = (next: typeof view) => {
    setView(next);
    setSelectedId(
      next === "projects"
        ? (config.projects[0]?.id ?? "")
        : (config.templates[0]?.id ?? ""),
    );
    setPaneId("");
  };
  const create = (kind: typeof view) => {
    if (kind === "templates") {
      const item: Template = {
        id: uid(),
        name: "Nouveau modèle",
        layout: newPane(),
      };
      mutate((data) => ({ ...data, templates: [...data.templates, item] }));
      setSelectedId(item.id);
    } else {
      let first = config.templates[0];
      if (!first)
        first = { id: uid(), name: "Terminal simple", layout: newPane() };
      const item: Project = {
        id: uid(),
        name: "Nouveau projet",
        root: "/home/",
        distribution: "",
        templateId: first.id,
        overrides: {},
      };
      mutate((data) => ({
        ...data,
        templates: data.templates.length ? data.templates : [first],
        projects: [...data.projects, item],
      }));
      setSelectedId(item.id);
    }
    setView(kind);
    setPaneId("");
  };
  const reorder = (id: string, targetId: string) =>
    mutate((data) =>
      view === "projects"
        ? { ...data, projects: moveItem(data.projects, id, targetId) }
        : { ...data, templates: moveItem(data.templates, id, targetId) },
    );
  const duplicate = () => {
    if (project) {
      const copy = {
        ...structuredClone(project),
        id: uid(),
        name: `${project.name} — copie`,
      };
      mutate((data) => ({ ...data, projects: [...data.projects, copy] }));
      setSelectedId(copy.id);
    } else if (template) {
      const copy = {
        ...template,
        id: uid(),
        name: `${template.name} — copie`,
        layout: duplicateLayout(template.layout),
      };
      mutate((data) => ({ ...data, templates: [...data.templates, copy] }));
      setSelectedId(copy.id);
    }
  };
  const remove = () => {
    if (project)
      mutate((data) => ({
        ...data,
        projects: data.projects.filter((p) => p.id !== project.id),
      }));
    else if (template) {
      if (config.projects.some((p) => p.templateId === template.id)) {
        setError(
          "Ce modèle est utilisé par un projet. Réaffectez ses projets avant de le supprimer.",
        );
        setDeleting(false);
        return;
      }
      mutate((data) => ({
        ...data,
        templates: data.templates.filter((t) => t.id !== template.id),
      }));
    }
    setSelectedId("");
    setDeleting(false);
  };
  const save = async (launch = false) => {
    if (busy) return;
    const invalid = validateConfig(config);
    if (invalid) {
      setError(invalid);
      return;
    }
    const snapshot = config;
    setBusy(launch ? "launch" : "save");
    setError("");
    setNotice("");
    try {
      await api.save(snapshot);
      setBaseline(JSON.stringify(snapshot));
      if (launch && project) {
        await api.launch(snapshot, project.id);
        setNotice("Demande de lancement transmise à Windows Terminal.");
      } else setNotice("Modifications enregistrées.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };
  const split = (axis: "columns" | "rows") => {
    if (!selectedPane || allPanes.length >= 16) return;
    const next = newPane(`Terminal ${allPanes.length + 1}`);
    changeLayout((layout) =>
      updateNode(layout, selectedPane.id, (node) => ({
        kind: "split",
        id: uid(),
        axis,
        ratio: 0.5,
        first: node,
        second: next,
      })),
    );
    setPaneId(next.id);
  };
  const checkUpdate = async () => {
    setCheckingUpdate(true);
    setError("");
    setNotice("");
    try {
      const found = await api.checkUpdate();
      if (!alive.current) return;
      setUpdate(found);
      if (!found) setNotice("RunTerm est à jour.");
    } catch (e) {
      if (alive.current)
        setError(`Impossible de vérifier les mises à jour : ${String(e)}`);
    } finally {
      if (alive.current) setCheckingUpdate(false);
    }
  };
  const installUpdate = async () => {
    // The installer closes RunTerm without going through onCloseRequested.
    if (!update || dirty) return;
    setError("");
    setUpdateProgress(null);
    try {
      await update.install((percent) => {
        if (alive.current) setUpdateProgress(percent);
      });
    } catch (e) {
      if (!alive.current) return;
      setUpdateProgress(undefined);
      setError(`La mise à jour a échoué : ${String(e)}`);
    }
  };
  const list = view === "projects" ? config.projects : config.templates;
  const custom = project && selectedPane && project.overrides[selectedPane.id];
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <Terminal size={22} />
          </span>
          <span>
            runterm<span className="brand-period">.</span>
          </span>
          <span className="version">BETA</span>
        </div>
        <div className="sidebar-caption">VOTRE ESPACE DE TRAVAIL</div>
        <nav aria-label="Navigation principale">
          <button
            className={view === "projects" ? "nav-item active" : "nav-item"}
            onClick={() => chooseView("projects")}
          >
            <FolderOpen size={18} /> Projets{" "}
            <span>{config.projects.length}</span>
          </button>
          <button
            className={view === "templates" ? "nav-item active" : "nav-item"}
            onClick={() => chooseView("templates")}
          >
            <LayoutTemplate size={18} /> Modèles de layout{" "}
            <span>{config.templates.length}</span>
          </button>
        </nav>
        <div className="list-heading">
          <span>{view === "projects" ? "MES PROJETS" : "MES MODÈLES"}</span>
          <button
            className="icon-button"
            aria-label={
              view === "projects" ? "Créer un projet" : "Créer un modèle"
            }
            onClick={() => create(view)}
          >
            <Plus size={16} />
          </button>
        </div>
        <div className="sidebar-list">
          {list.map((item) => (
            <button
              key={item.id}
              className={`sidebar-entry ${selectedId === item.id ? "current" : ""} ${dragId === item.id ? "dragging" : ""}`}
              title="Glisser pour réordonner (ou Alt + ↑ / ↓)"
              draggable
              onDragStart={(e) => {
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", item.id);
                setDragId(item.id);
              }}
              onDragOver={(e) => {
                if (!dragId) return;
                e.preventDefault();
                if (dragId !== item.id) reorder(dragId, item.id);
              }}
              onDrop={(e) => e.preventDefault()}
              onDragEnd={() => setDragId("")}
              onKeyDown={(e) => {
                if (!e.altKey) return;
                const index = list.findIndex((other) => other.id === item.id);
                const target =
                  e.key === "ArrowUp"
                    ? list[index - 1]
                    : e.key === "ArrowDown"
                      ? list[index + 1]
                      : undefined;
                if (!target) return;
                e.preventDefault();
                reorder(item.id, target.id);
              }}
              onClick={() => {
                setSelectedId(item.id);
                setPaneId("");
              }}
            >
              <span className="entry-icon">
                {view === "projects" ? (
                  <Folder size={16} />
                ) : (
                  <Layers size={16} />
                )}
              </span>
              <span>{item.name}</span>
              {selectedId === item.id && <ChevronRight size={14} />}
            </button>
          ))}
          {!list.length && (
            <p className="sidebar-empty">Vos projets, prêts à démarrer.</p>
          )}
        </div>
        <div className="sidebar-bottom">
          <span className="status-dot" />
          <div>
            Windows Terminal + WSL
            <small>
              {desktop ? "Environnement local" : "Aperçu navigateur"}
              {version && ` · v${version}`}
            </small>
          </div>
          {desktop && (
            <button
              className="icon-button update-check"
              aria-label="Rechercher une mise à jour"
              title="Rechercher une mise à jour"
              disabled={checkingUpdate || updateProgress !== undefined}
              onClick={checkUpdate}
            >
              <RefreshCw size={14} className={checkingUpdate ? "spin" : ""} />
            </button>
          )}
        </div>
      </aside>
      <main className="main">
        <header className="topbar">
          <div className="breadcrumb">
            <span>Workspace</span>
            <ChevronRight size={14} />
            <strong>{view === "projects" ? "Projets" : "Modèles"}</strong>
          </div>
          <div className="save-status">
            <span className={dirty ? "unsaved-dot" : "status-dot"} />
            {dirty ? "Modifications non enregistrées" : "À jour"}
          </div>
        </header>
        {update && (
          <div className="update-banner" role="status">
            <span>
              <strong>RunTerm {update.version} est disponible</strong> (version
              actuelle {update.currentVersion}).
              {dirty &&
                updateProgress === undefined &&
                " Enregistrez vos modifications avant de l’installer."}
            </span>
            {updateProgress === undefined ? (
              <span className="update-actions">
                <button
                  className="button primary small"
                  disabled={dirty}
                  onClick={installUpdate}
                >
                  <Download size={14} /> Installer et redémarrer
                </button>
                <button
                  className="icon-button"
                  aria-label="Ignorer la mise à jour"
                  onClick={() => setUpdate(null)}
                >
                  <X size={15} />
                </button>
              </span>
            ) : (
              <span className="update-actions">
                <LoaderCircle className="spin" size={14} />
                {updateProgress === null
                  ? "Téléchargement…"
                  : `Téléchargement… ${Math.round(updateProgress)} %`}
              </span>
            )}
          </div>
        )}
        {!desktop && (
          <div className="preview-banner">
            Aperçu de l’éditeur · Les configurations sont sauvegardées dans ce
            navigateur. Le lancement nécessite l’application Windows.
          </div>
        )}
        <div className="page-content">
          <div className="page-heading">
            <div>
              <div className="eyebrow">CONFIGURER. LANCER. CRÉER.</div>
              <h1>
                {view === "projects"
                  ? "Vos projets, en place."
                  : "Un layout. Mille départs."}
              </h1>
              <p>
                {view === "projects"
                  ? "Retrouvez vos outils et vos terminaux, exactement où vous les voulez."
                  : "Composez un espace de travail et réutilisez-le dans tous vos projets."}
              </p>
            </div>
            <button className="button secondary" onClick={() => create(view)}>
              <Plus size={16} />
              {view === "projects" ? "Nouveau projet" : "Nouveau modèle"}
            </button>
          </div>
          {(error || notice) && (
            <div
              ref={message}
              role={error ? "alert" : "status"}
              className={`message ${error ? "error" : "success"}`}
            >
              {error || notice}
              <button
                className="icon-button"
                aria-label="Fermer le message"
                onClick={() => {
                  setError("");
                  setNotice("");
                }}
              >
                <X size={15} />
              </button>
            </div>
          )}
          {!template ? (
            <section className="empty-state">
              <span className="empty-icon">
                <PanelsIllustration />
              </span>
              <span className="eyebrow">UN BON DÉPART, À CHAQUE FOIS</span>
              <h2>
                {view === "projects"
                  ? "Votre prochain projet commence ici."
                  : "Dessinez votre espace de travail."}
              </h2>
              <p>
                {view === "projects"
                  ? "Choisissez un dossier, associez-lui un layout et retrouvez tous vos terminaux en un clic."
                  : "Divisez les panneaux, ajoutez vos commandes et gardez votre configuration préférée."}
              </p>
              <button className="button primary" onClick={() => create(view)}>
                <Plus size={17} />
                {view === "projects"
                  ? "Créer mon premier projet"
                  : "Créer un modèle"}
              </button>
              {view === "projects" && config.templates[0] && (
                <button
                  className="text-button"
                  onClick={() => chooseView("templates")}
                >
                  Explorer le modèle développeur <ChevronRight size={15} />
                </button>
              )}
            </section>
          ) : (
            <>
              <section className="config-card">
                <div className="section-label">
                  <span className="section-number">01</span>
                  {project ? "Le projet" : "Le modèle"}
                  <div className="spacer" />
                  <button
                    className="icon-button"
                    title="Dupliquer"
                    aria-label="Dupliquer"
                    onClick={duplicate}
                  >
                    <Copy size={16} />
                  </button>
                  <button
                    className="icon-button danger"
                    title="Supprimer"
                    aria-label="Supprimer"
                    onClick={() => setDeleting(true)}
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
                <div
                  className={`config-fields ${project ? "" : "template-fields"}`}
                >
                  <label>
                    Nom {project ? "du projet" : "du modèle"}
                    <input
                      value={project?.name ?? template.name}
                      onChange={(e) =>
                        project
                          ? editProject({ name: e.target.value })
                          : editTemplate({ name: e.target.value })
                      }
                    />
                  </label>
                  {project ? (
                    <>
                      <label className="root-field">
                        Dossier racine WSL
                        <input
                          className="mono"
                          placeholder="/home/utilisateur/workspaces/projet"
                          value={project.root}
                          onChange={(e) =>
                            editProject({ root: e.target.value })
                          }
                        />
                      </label>
                      <label>
                        Distribution
                        <select
                          value={project.distribution}
                          onChange={(e) =>
                            editProject({ distribution: e.target.value })
                          }
                        >
                          <option value="">Distribution par défaut</option>
                          {[
                            ...new Set([
                              ...distributions,
                              ...(project.distribution
                                ? [project.distribution]
                                : []),
                            ]),
                          ].map((d) => (
                            <option key={d}>{d}</option>
                          ))}
                        </select>
                      </label>
                      <label>
                        Profil Windows Terminal
                        <input
                          placeholder={`Automatique (${
                            project.distribution ||
                            distributions[0] ||
                            "distribution par défaut"
                          })`}
                          title="Nom ou GUID d’un profil Windows Terminal, pour ses couleurs et sa police. Vide : le profil portant le nom de la distribution."
                          value={project.terminalProfile ?? ""}
                          onChange={(e) =>
                            editProject({
                              terminalProfile: e.target.value || undefined,
                            })
                          }
                        />
                      </label>
                      <label>
                        Modèle de layout
                        <select
                          value={project.templateId}
                          onChange={(e) => {
                            editProject({
                              templateId: e.target.value,
                              overrides: {},
                            });
                            setPaneId("");
                          }}
                        >
                          {config.templates.map((t) => (
                            <option key={t.id} value={t.id}>
                              {t.name}
                            </option>
                          ))}
                        </select>
                      </label>
                    </>
                  ) : (
                    <div className="template-help">
                      <Layers size={20} />
                      <p>
                        Les projets liés suivent les modifications de ce modèle.
                        <br />
                        <span>
                          Leurs commandes personnalisées sont conservées.
                        </span>
                      </p>
                    </div>
                  )}
                </div>
              </section>
              <section className="workspace-card">
                <div className="section-label">
                  <span className="section-number">02</span>Disposition des
                  panneaux{" "}
                  <span className="count-badge">
                    {allPanes.length} panneaux
                  </span>
                  <div className="spacer" />
                  {project ? (
                    <button
                      className="text-button"
                      onClick={() => {
                        setView("templates");
                        setSelectedId(template.id);
                      }}
                    >
                      Modifier le modèle <ChevronRight size={14} />
                    </button>
                  ) : (
                    <span className="subtle">
                      Déplacez les séparateurs pour ajuster
                    </span>
                  )}
                </div>
                <div className="editor-body">
                  <div className="canvas-column">
                    <div className="canvas-toolbar">
                      <span>
                        <span className="status-dot" />{" "}
                        {project?.name ?? template.name}
                      </span>
                      <span className="terminal-label">BASH / WSL</span>
                    </div>
                    <div className="layout-canvas">
                      <LayoutPreview
                        node={template.layout}
                        selected={selectedPane?.id}
                        onSelect={setPaneId}
                        onRatio={
                          project
                            ? undefined
                            : (id, ratio) =>
                                changeLayout((layout) =>
                                  updateNode(layout, id, (node) =>
                                    node.kind === "split"
                                      ? { ...node, ratio }
                                      : node,
                                  ),
                                )
                        }
                        project={project}
                      />
                    </div>
                    <div className="canvas-footer">
                      <Terminal size={14} />
                      <span>
                        Chaque panneau ouvre son propre shell interactif.
                      </span>
                    </div>
                    {!project && (
                      <div className="split-tools">
                        <button
                          className="button secondary small"
                          disabled={allPanes.length >= 16}
                          onClick={() => split("columns")}
                        >
                          <Columns2 size={16} />
                          Gauche / droite
                        </button>
                        <button
                          className="button secondary small"
                          disabled={allPanes.length >= 16}
                          onClick={() => split("rows")}
                        >
                          <Rows2 size={16} />
                          Haut / bas
                        </button>
                        <button
                          className="icon-button danger"
                          disabled={allPanes.length === 1}
                          aria-label="Supprimer le panneau"
                          onClick={() => {
                            if (selectedPane) {
                              changeLayout((layout) =>
                                removePane(layout, selectedPane.id),
                              );
                              setPaneId("");
                            }
                          }}
                        >
                          <PanelLeftClose size={17} />
                        </button>
                      </div>
                    )}
                  </div>
                  {effective && (
                    <aside className="inspector">
                      <div className="inspector-title">
                        <Terminal size={17} />
                        <h3>{effective.name}</h3>
                        <span className="tiny-badge">
                          {custom
                            ? "PERSONNALISÉ"
                            : project
                              ? "HÉRITÉ"
                              : "PANNEAU"}
                        </span>
                      </div>
                      {!project && (
                        <label>
                          Nom du panneau
                          <input
                            value={effective.name}
                            onChange={(e) => editPane({ name: e.target.value })}
                          />
                        </label>
                      )}
                      <label>
                        Répertoire relatif
                        <input
                          className="mono"
                          value={effective.directory}
                          onChange={(e) =>
                            editPane({ directory: e.target.value })
                          }
                          placeholder=". ou backend"
                        />
                      </label>
                      <p className="field-hint">
                        Relatif au dossier du projet. <code>.</code> utilise sa
                        racine.
                      </p>
                      <div className="actions-heading">
                        <label>Actions au lancement</label>
                        <span>{effective.commands.length}</span>
                      </div>
                      <div className="command-list">
                        {effective.commands.map((command, index) => (
                          <div className="command-item" key={index}>
                            <div className="command-meta">
                              <span>
                                <span className="prompt">❯</span> ACTION{" "}
                                {String(index + 1).padStart(2, "0")}
                              </span>
                              <div>
                                <button
                                  className="icon-button"
                                  disabled={index === 0}
                                  aria-label={`Monter l’action ${index + 1}`}
                                  onClick={() => {
                                    const commands = [...effective.commands];
                                    [commands[index - 1], commands[index]] = [
                                      commands[index],
                                      commands[index - 1],
                                    ];
                                    editPane({ commands });
                                  }}
                                >
                                  <ArrowUp size={13} />
                                </button>
                                <button
                                  className="icon-button"
                                  disabled={
                                    index === effective.commands.length - 1
                                  }
                                  aria-label={`Descendre l’action ${index + 1}`}
                                  onClick={() => {
                                    const commands = [...effective.commands];
                                    [commands[index + 1], commands[index]] = [
                                      commands[index],
                                      commands[index + 1],
                                    ];
                                    editPane({ commands });
                                  }}
                                >
                                  <ArrowDown size={13} />
                                </button>
                                <button
                                  className="icon-button"
                                  aria-label={`Supprimer l’action ${index + 1}`}
                                  onClick={() =>
                                    editPane({
                                      commands: effective.commands.filter(
                                        (_, i) => i !== index,
                                      ),
                                    })
                                  }
                                >
                                  <X size={13} />
                                </button>
                              </div>
                            </div>
                            <textarea
                              aria-label={`Commande ${index + 1}`}
                              value={command}
                              spellCheck={false}
                              placeholder="Votre commande Bash…"
                              onChange={(e) =>
                                editPane({
                                  commands: effective.commands.map((c, i) =>
                                    i === index ? e.target.value : c,
                                  ),
                                })
                              }
                            />
                          </div>
                        ))}
                      </div>
                      <button
                        className="add-action"
                        onClick={() =>
                          editPane({ commands: [...effective.commands, ""] })
                        }
                      >
                        <Plus size={15} />
                        Ajouter une action
                      </button>
                      <p className="field-hint execution-hint">
                        Les actions s’exécutent dans l’ordre. Un serveur ou une
                        commande interactive attend sa fin avant la suite. En
                        cas d’échec, le shell reste ouvert.
                      </p>
                      {custom && (
                        <button
                          className="text-button reset"
                          onClick={() => {
                            const overrides = { ...project!.overrides };
                            delete overrides[selectedPane!.id];
                            editProject({ overrides });
                          }}
                        >
                          <RotateCcw size={14} />
                          Revenir aux valeurs du modèle
                        </button>
                      )}
                    </aside>
                  )}
                </div>
              </section>
              <footer className="action-footer">
                <span>
                  <Check size={15} />
                  Sauvegardé localement, disponible à tout moment.
                </span>
                <div>
                  {dirty && (
                    <button
                      className="text-button discard"
                      onClick={() => {
                        setConfig(JSON.parse(baseline) as Config);
                        setError("");
                      }}
                    >
                      Annuler les modifications
                    </button>
                  )}
                  <button
                    className="button secondary"
                    disabled={!!busy}
                    onClick={() => void save()}
                  >
                    {busy === "save" ? (
                      <LoaderCircle className="spin" size={16} />
                    ) : (
                      <Save size={16} />
                    )}
                    Enregistrer
                  </button>
                  {project && (
                    <button
                      className="button primary"
                      disabled={!!busy || !desktop}
                      title={
                        !desktop
                          ? "Disponible dans l’application Windows"
                          : "Enregistrer et lancer le projet"
                      }
                      onClick={() => void save(true)}
                    >
                      {busy === "launch" ? (
                        <LoaderCircle className="spin" size={16} />
                      ) : (
                        <Play size={16} />
                      )}
                      Lancer le projet
                    </button>
                  )}
                </div>
              </footer>
            </>
          )}
          {!template && dirty && (
            <div className="empty-save">
              <button
                className="button secondary"
                disabled={!!busy}
                onClick={() => void save()}
              >
                <Save size={16} />
                Enregistrer les modifications
              </button>
            </div>
          )}
        </div>
      </main>
      {deleting && (
        <div className="modal-backdrop">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="delete-title"
            className="modal"
          >
            <Trash2 size={25} />
            <h2 id="delete-title">
              Supprimer « {project?.name ?? template?.name} » ?
            </h2>
            <p>La suppression sera définitive après enregistrement.</p>
            <div>
              <button
                autoFocus
                className="button secondary"
                onClick={() => setDeleting(false)}
              >
                Annuler
              </button>
              <button className="button destructive" onClick={remove}>
                Supprimer
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
function PanelsIllustration() {
  return (
    <div className="panels-illustration">
      <div>
        <span />
        <span />
        <span />
      </div>
      <section>
        <article>
          <Terminal size={22} />
          <i />
          <i />
        </article>
        <article>
          <i />
          <i />
          <i />
        </article>
        <article>
          <i />
          <i />
        </article>
      </section>
    </div>
  );
}
