# Todolist d'implémentation — devtodo

## Phase 1 — Fondations

- [x] **#1** Initialiser le projet Rust avec Cargo
  - `cargo init devtodo`, configurer `Cargo.toml` avec toutes les dépendances (clap, rusqlite, reqwest, tokio, serde, etc.)

- [x] **#2** Créer le module `error.rs` *(bloqué par #1)*
  - `DevTodoError` avec `thiserror` : Db, Api, Config, NotFound, InvalidStatus, Git, Io, Serialization

- [x] **#3** Créer le module `models.rs` *(bloqué par #1)*
  - Structs : Task, Label, Reviewer, Comment, TaskStatus, Priority avec serde

---

## Phase 2 — Infrastructure

- [x] **#4** Créer le module `db.rs` *(bloqué par #2, #3)*
  - Couche SQLite : `init_db()`, création des 5 tables, fonctions CRUD pour chaque entité

- [x] **#5** Créer le module `cli.rs` avec clap derive *(bloqué par #3)*
  - Toutes les commandes/sous-commandes : init, add, list, show, edit, status, delete, label, review, sync, push, pull, stats, export, config

- [x] **#6** Créer le module `display.rs` *(bloqué par #3)*
  - Formatage tableaux avec `comfy-table`, statuts et priorités colorés avec `colored`

---

## Phase 3 — Commandes locales

- [x] **#7** Implémenter la commande `init` *(bloqué par #4, #5)*
  - Créer `.devtodo.db`, exécuter migrations, détecter le remote Git

- [x] **#8** Implémenter la commande `add` *(bloqué par #4, #5)*
  - Créer une tâche avec titre, description, priorité, branche, labels, assignee

- [x] **#9** Implémenter la commande `list` *(bloqué par #4, #5, #6)*
  - Lister les tâches avec filtres : --status, --label, --priority, --assignee, --sort, --limit

- [x] **#10** Implémenter la commande `show` *(bloqué par #4, #5, #6)*
  - Détails d'une tâche avec --comments et --json

- [x] **#11** Implémenter la commande `edit` *(bloqué par #4, #5)*
  - Modifier titre, description, priorité, branche, assignee

- [x] **#12** Implémenter la commande `status` *(bloqué par #4, #5)*
  - Changer le statut : draft | open | review | merged | closed

- [x] **#13** Implémenter la commande `delete` *(bloqué par #4, #5)*
  - Supprimer avec confirmation (dialoguer), --force, suppression en cascade

- [x] **#14** Implémenter les commandes `label` *(bloqué par #4, #5)*
  - Sous-commandes : add, remove, list, assign, unassign

- [x] **#15** Implémenter les commandes `review` *(bloqué par #4, #5)*
  - Sous-commandes : assign, remove, status, list

- [x] **#16** Implémenter la commande `config` *(bloqué par #2)*
  - set, get, list — stockage `~/.config/devtodo/config.toml`, masquage des tokens, permissions 600

- [x] **#17** Implémenter la commande `stats` *(bloqué par #4, #5)*
  - Stats par statut, priorité, label, temps moyen de merge, activité par semaine, --period

- [x] **#18** Implémenter la commande `export` *(bloqué par #4, #5)*
  - Formats : JSON, CSV, Markdown — options --output et --status

---

## Phase 4 — Providers & Synchronisation

- [x] **#19** Créer le trait Provider + implémenter GitHub *(bloqué par #2, #3)*
  - Trait Provider : list_prs, create_pr, update_pr, list_reviews, list_comments, list_labels, assign_labels
  - GitHub REST API v3, mapping des statuts

- [x] **#20** Implémenter le provider GitLab *(bloqué par #19)*
  - GitLab REST API v4, support instances self-hosted, mapping statuts MR

- [x] **#21** Implémenter les commandes `sync`/`push`/`pull` *(bloqué par #4, #19, #20)*
  - sync bidirectionnelle (--provider, --dry-run), push PR distante, pull/import PRs, barre de progression

---

## Phase 5 — Finalisation

- [x] **#22** Câbler `main.rs` et le dispatch des commandes *(bloqué par #5, #7)*
  - Parser les args CLI, ouvrir la DB, dispatcher vers les commandes, setup tokio runtime

- [x] **#23** Ajouter l'autocomplétion shell *(bloqué par #5)*
  - `clap_complete` pour bash, zsh, fish — commande `devtodo completions <shell>`

- [x] **#24** Tests et compilation finale *(bloqué par #22)*
  - `cargo build`, corriger les erreurs, tests unitaires pour db, models, providers
