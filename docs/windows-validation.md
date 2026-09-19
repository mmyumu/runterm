# Recette Windows

À exécuter dans l’application Tauri Windows, avec une distribution WSL de test. Les tests automatisés Linux couvrent l’éditeur et le moteur, mais ne démontrent pas le comportement de Windows Terminal.

1. Créer dans WSL deux projets temporaires contenant chacun `backend/` et `frontend/`.
2. Dupliquer le modèle développeur. Remplacer Codex et Claude par `printf 'assistant prêt\n'`, et Uvicorn par `python3 -m http.server 8765` (ou un port libre).
3. Associer les deux projets au même modèle. Lancer chacun séparément et vérifier les cinq panneaux, leurs noms, leurs dossiers (`pwd`) et la disposition : deux panneaux en haut ; shell sur la moitié basse gauche ; backend et frontend dans les deux quarts restants.
4. Diviser encore un panneau dans le modèle, changer les proportions puis relancer : la disposition doit suivre les changements. Retirer ce panneau et vérifier que son voisin récupère l’espace.
5. Personnaliser une commande uniquement dans le second projet. Modifier ensuite le modèle : le premier projet hérite, le second conserve sa commande. Utiliser le bouton de réinitialisation pour rétablir l’héritage.
6. Dans un panneau, configurer trois actions : `export RUNTERM_CHECK='a ; b'`, `printf '%s\n' "$RUNTERM_CHECK"`, `pwd`. Vérifier l’ordre et l’environnement partagé ; le prompt final reste utilisable.
7. Configurer `false` puis `echo NE_DOIT_PAS_APPARAITRE`. Vérifier l’arrêt, le message d’erreur et le prompt final.
8. Ajouter une action après le serveur HTTP. Interrompre le serveur avec Ctrl+C : le prompt revient, l’action suivante ne démarre pas. Vérifier aussi qu’un programme terminé normalement autorise la suite.
9. Tester une racine avec espaces et apostrophe, ainsi que des commandes contenant guillemets, `$()`, points-virgules et plusieurs lignes. Seules les commandes saisies doivent être interprétées comme du Bash.
10. Sélectionner une distribution absente (en modifiant une copie du JSON) ou un dossier inexistant. L’application doit afficher une erreur sans ouvrir un layout partiel.
11. Enregistrer, fermer et rouvrir RunTerm : projets, modèles et personnalisations sont identiques. Fermer RunTerm après lancement laisse les terminaux actifs.
12. Sur une copie de sauvegarde, rendre le JSON invalide puis relancer : la GUI doit refuser l’ouverture sans remplacer le fichier.
13. Construire l’installateur avec `npm run tauri -- build`, l’installer et vérifier le démarrage et un lancement WSL depuis le raccourci de l’application.

Le message de réussite signifie que la demande a été transmise à Windows Terminal, pas que chaque serveur a démarré correctement. Les erreurs des programmes restent visibles dans leurs panneaux.

## Test automatique de lancement réel

Dans une session Windows interactive, après `npm run build` :

```powershell
cargo test -p runterm windows_terminal_smoke -- --ignored --nocapture
```

Ce test ouvre temporairement une fenêtre de cinq panneaux dans la distribution WSL par défaut. Il vérifie les répertoires et les variables partagées entre actions grâce à des fichiers temporaires, puis ferme ses shells. Il ne lance ni Codex, ni Claude, ni les serveurs du projet. Il ne vérifie pas visuellement la géométrie des panneaux.
