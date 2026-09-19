# RunTerm

Une petite application Windows pour composer, sauvegarder et lancer des espaces de travail dans **Windows Terminal + WSL**.

Interface française en React/TypeScript, application Tauri 2 et moteur Rust. Aucun serveur ni compte nécessaire.

## Version Windows prête à lancer

Les fichiers compilés de cette session sont disponibles dans `artifacts/` :

- `RunTerm_0.1.0_x64-setup.exe` : installateur Windows x64.
- `RunTerm.exe` : exécutable autonome, utilisable avec WebView2 déjà installé.
- `SHA256SUMS.txt` : empreintes des deux fichiers.

Lancez ces fichiers depuis Windows. Le binaire est compilé et testé ; il n’est pas signé. Le bilan des vérifications est dans [docs/validation.md](docs/validation.md).

## Utilisation

1. Dans **Modèles de layout**, choisissez le modèle développeur fourni ou créez le vôtre.
2. Sélectionnez un panneau, divisez-le gauche/droite ou haut/bas et déplacez les séparateurs. Les flèches du clavier ajustent aussi un séparateur sélectionné.
3. Configurez le nom du panneau, son dossier relatif (`.`, `backend`, `frontend`…) et ses actions Bash, dans l’ordre voulu.
4. Dans **Projets**, créez un projet, indiquez sa racine Linux absolue, sa distribution WSL et son modèle. Le **profil Windows Terminal** (nom ou GUID, optionnel) donne aux panneaux ses couleurs et sa police ; vide, RunTerm utilise le profil portant le nom de la distribution.
5. Personnalisez les commandes ou dossiers pour ce projet si nécessaire. **Revenir aux valeurs du modèle** rétablit l’héritage du panneau.
6. **Lancer le projet** enregistre la configuration, vérifie les dossiers puis ouvre une nouvelle fenêtre Windows Terminal.

Le modèle fourni correspond à l’exemple : Codex et Claude en haut, shell libre, backend Uvicorn et shell frontend en bas. Les outils doivent être installés dans la distribution WSL ; RunTerm ne les installe pas.

Chaque panneau a son propre shell. Ses actions partagent les changements de dossier et d’environnement. Une commande interactive ou un serveur bloque la suite jusqu’à sa fin. Un échec ou Ctrl+C arrête la séquence et laisse le prompt disponible. `exit` ferme volontairement le shell. Les panneaux démarrent indépendamment : aucun mécanisme d’attente entre services.

Modifier un modèle affecte ses projets au prochain lancement ; les champs personnalisés restent prioritaires. Changer de modèle dans un projet remet ses personnalisations à zéro. Un modèle utilisé ne peut pas être supprimé. Maximum : 16 panneaux, avec des proportions de 10 à 90 % ; la taille minimale de Windows Terminal peut limiter les dispositions très denses.

## Développement Windows

Pré-requis :

- Windows 10/11 avec Windows Terminal (`wt.exe` accessible dans le PATH), WSL et une distribution disposant de Bash.
- Node.js 22.12 ou supérieur et npm.
- Rust via rustup ; `rust-toolchain.toml` sélectionne Rust 1.94.0.
- Visual Studio Build Tools avec **Développement Desktop en C++** et Windows SDK.
- Microsoft Edge WebView2 Runtime.

Voir les [prérequis officiels Tauri](https://v2.tauri.app/start/prerequisites/).

Dans **PowerShell Windows**, dans un checkout situé sur un disque Windows (par exemple `C:\dev\runterm`) :

```powershell
npm ci
npm run tauri -- dev
```

Ne partagez pas `node_modules` ou `target` entre les installations Linux et Windows. Le projet courant peut être développé sous WSL, mais la compilation native et l’installateur se construisent avec les outils Windows.

## Générer l’installateur

```powershell
npm run tauri -- build
```

Depuis WSL, `scripts/build-windows.sh` fait la même chose avec les outils Windows : il copie les sources dans `%USERPROFILE%\runterm-build` (modifiable via `RUNTERM_WIN_BUILD_DIR`), y lance `scripts\build-windows.cmd`, puis place `RunTerm.exe`, l’installateur et `SHA256SUMS.txt` dans `artifacts/`. Visual Studio Build Tools restent requis côté Windows ; Node et Rust peuvent être installés sous Windows ou fournis en version portable dans les dossiers `node\` et `toolchain\` du dossier de build.

L’installateur NSIS se trouve dans `target\release\bundle\nsis\`. Il n’est pas signé ; une signature de distribution n’est pas configurée. La configuration Tauri prévoit l’installation de WebView2 si nécessaire. Windows Terminal et WSL restent des prérequis à installer séparément.

Le workflow GitHub Actions **Checks and Windows installer** construit et conserve l’installateur comme artefact, sans publier de release. Il se lance sur push, pull request ou manuellement une fois le dépôt hébergé sur GitHub.

## Aperçu de l’interface sous WSL/Linux

```bash
npm ci
npm run dev
```

Ouvrir `http://localhost:1420`. L’aperçu sauvegarde dans le stockage local du navigateur et désactive le lancement. Ses données ne sont pas transférées à l’application Windows.

## Vérifications

```bash
npm run check
npm run format:check
cargo test -p runterm-core
cargo clippy -p runterm-core --all-targets -- -D warnings
cargo fmt --all --check
```

Sur Windows, vérifier aussi `cargo check -p runterm`. Depuis WSL, une vérification de types ciblant Windows est possible avec `rustup target add x86_64-pc-windows-msvc` puis `cargo check -p runterm --target x86_64-pc-windows-msvc` ; elle ne remplace pas l’édition de liens, la création de l’installateur ou le test réel de Windows Terminal. La recette manuelle figure dans [docs/windows-validation.md](docs/windows-validation.md).

## Données et architecture

- `src/` : éditeur, projets, modèles et client des commandes Tauri. L’aperçu navigateur utilise un stockage séparé.
- `crates/core/` : schéma JSON versionné, validation, résolution des personnalisations, sauvegarde atomique, scripts Bash et compilation du layout en arguments Windows Terminal.
- `src-tauri/` : intégration Windows, découverte WSL et exécution des programmes avec arguments séparés. Aucune commande utilisateur ne transite par `cmd.exe`.

L’application sauvegarde dans `%APPDATA%\dev.runterm.desktop\config.json`. Une configuration corrompue ou d’une version inconnue bloque sa réécriture : le fichier est conservé. Fermer une fenêtre avec des modifications non enregistrées demande de les enregistrer ou de les annuler.

Les scripts temporaires sont créés avec permissions privées dans `/tmp/runterm-*` de la distribution choisie. Chaque panneau supprime son script au démarrage ; le dernier retire le dossier. Une erreur de préparation déclenche le nettoyage. Si Windows Terminal échoue après transmission, des scripts peuvent rester dans `/tmp` jusqu’au nettoyage de la distribution.

Les commandes saisies sont du code Bash exécuté avec les droits de l’utilisateur WSL. Le fichier JSON contient les commandes en clair : les secrets doivent rester dans l’environnement ou les outils habituels du projet.

La première version ouvre un onglet par lancement. Elle ne pilote pas les panneaux déjà ouverts, ne suit pas l’état des serveurs et n’importe pas les raccourcis batch. Fermer RunTerm laisse Windows Terminal fonctionner.

## Licence

MIT, voir [LICENSE](LICENSE).
