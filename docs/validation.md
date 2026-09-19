# Vérifications effectuées — 19 septembre 2026

## Résultats

- Compilation frontend TypeScript/Vite réussie.
- 6 tests frontend réussis : édition, sauvegarde et rechargement d’un projet personnalisé ; division/suppression de panneau ; protection d’un modèle utilisé ; conservation d’un stockage corrompu ; héritage et identifiants.
- 7 tests Rust sous Linux réussis : sauvegarde atomique et refus de corruption ; résolution des projets ; génération des arguments de layouts imbriqués ; validation ; séquences Bash ; Ctrl+C avec un vrai pseudo-terminal ; conservation des hooks de prompt et exécution unique des actions.
- Vérification Clippy sans avertissement pour le moteur et le code Windows, y compris les tests.
- Formatage Rust et frontend vérifiés.
- 4 tests du moteur exécutés aussi avec Rust Windows : tous réussis.
- 2 tests d’intégration Windows réussis : décodage UTF-16/UTF-8 et lancement réel Windows Terminal/WSL.
- Test réel : cinq panneaux exécutent leurs actions dans le bon dossier et partagent une variable entre actions. Les shells de test se ferment ensuite automatiquement. Aucun assistant ni serveur du projet utilisateur n’est lancé.
- Interface vérifiée dans Chromium : création, sauvegarde, sélection des panneaux et ajustement d’un séparateur au clavier ; pas de débordement horizontal à 1280 px.
- Exécutable Windows lancé : la WebView charge `http://tauri.localhost/`, affiche l’interface française et dispose de l’API Tauri. L’interface embarquée ne dépend pas de Vite.
- Installateur NSIS Windows x64 généré avec succès. Les livrables et leurs empreintes sont dans `artifacts/`.
- L’audit npm effectué après mise à jour de Vitest ne rapporte aucune vulnérabilité.

## Limites de la validation

L’installation/désinstallation NSIS n’a pas été exécutée. L’application a été vérifiée directement depuis l’exécutable compilé. La géométrie réelle des cinq panneaux n’a pas fait l’objet d’une comparaison visuelle automatisée ; les arguments générés sont testés, et le démarrage des cinq shells est vérifié sur Windows Terminal.

Les dépendances Windows de compilation ont été utilisées depuis un dossier temporaire, avec le compilateur C++ déjà installé. La compilation native a utilisé le même code applicatif que le workspace. Un test Linux supplémentaire du prompt a été ajouté ensuite, sans modification du code applicatif.

La recette manuelle complémentaire est dans [windows-validation.md](windows-validation.md).

## Mise à jour : menu contextuel

Le menu contextuel de la WebView est désactivé globalement. Vérification dans Chromium : les événements `contextmenu` sur le fond de l’interface et sur un bouton sont annulés. Compilation frontend, 6 tests frontend et vérification du formatage réussies.
