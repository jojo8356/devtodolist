# Cahier des Charges — devtodo

## 1. Présentation générale

| Champ | Valeur |
|-------|--------|
| **Nom** | devtodo |
| **Type** | Application CLI |
| **Langage** | Rust |
| **Concept** | Todolist développeur où chaque tâche est modélisée comme une Pull Request |
| **Cible** | Développeurs individuels ou en équipe travaillant avec Git |

### 1.1 Objectif

Fournir un outil en ligne de commande permettant de gérer ses tâches de développement sous forme de pull-requests, avec synchronisation bidirectionnelle vers GitHub et GitLab. Chaque tâche possède un cycle de vie similaire à une vraie PR : brouillon, ouverte, en review, mergée ou fermée.

### 1.2 Problème résolu

- Centraliser le suivi des tâches dev directement depuis le terminal
- Lier chaque tâche à une branche Git et une PR distante
- Éviter les allers-retours entre le terminal et l'interface web GitHub/GitLab
- Avoir une vue unifiée des PRs provenant de plusieurs providers

---

## 2. Architecture technique

### 2.1 Stack

| Composant | Choix | Justification |
|-----------|-------|---------------|
| CLI framework | `clap` (derive API) | Standard Rust, ergonomique, autocomplétion |
| Base de données | SQLite via `rusqlite` | Léger, portable, pas de serveur |
| HTTP client | `reqwest` (async) | Mature, supporte TLS natif |
| Sérialisation | `serde` + `serde_json` | Standard de facto en Rust |
| Configuration | `toml` + `directories` | Idiomatique Rust, XDG-compliant |
| Affichage tableaux | `comfy-table` | Tableaux formatés dans le terminal |
| Couleurs terminal | `colored` | Sortie colorée pour statuts/priorités |
| Runtime async | `tokio` | Requis par reqwest pour les appels API |

### 2.2 Structure du projet

```
devtodo/
├── Cargo.toml
├── CDC.md
├── src/
│   ├── main.rs              # Point d'entrée, dispatch des commandes
│   ├── cli.rs               # Définition CLI avec clap (derive)
│   ├── db.rs                # Couche d'accès SQLite (init, migrations, CRUD)
│   ├── models.rs            # Structs : Task, Label, Reviewer, Config
│   ├── error.rs             # Type d'erreur unifié (thiserror)
│   ├── display.rs           # Formatage tableaux et affichage terminal
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── init.rs          # devtodo init
│   │   ├── add.rs           # devtodo add
│   │   ├── list.rs          # devtodo list
│   │   ├── show.rs          # devtodo show
│   │   ├── edit.rs          # devtodo edit
│   │   ├── status.rs        # devtodo status
│   │   ├── delete.rs        # devtodo delete
│   │   ├── label.rs         # devtodo label
│   │   ├── review.rs        # devtodo review
│   │   ├── sync.rs          # devtodo sync / push / pull
│   │   ├── stats.rs         # devtodo stats
│   │   ├── export.rs        # devtodo export
│   │   └── config.rs        # devtodo config
│   └── providers/
│       ├── mod.rs            # Trait Provider commun
│       ├── github.rs         # Implémentation API GitHub (REST v3)
│       └── gitlab.rs         # Implémentation API GitLab (REST v4)
```

---

## 3. Modèle de données

### 3.1 Table `tasks`

| Colonne | Type | Description |
|---------|------|-------------|
| `id` | INTEGER PK | Identifiant auto-incrémenté |
| `title` | TEXT NOT NULL | Titre de la tâche/PR |
| `description` | TEXT | Description détaillée (corps de la PR) |
| `status` | TEXT NOT NULL | `draft` \| `open` \| `review` \| `merged` \| `closed` |
| `priority` | TEXT | `low` \| `medium` \| `high` \| `critical` |
| `branch` | TEXT | Nom de la branche Git associée |
| `base_branch` | TEXT | Branche cible (ex: `main`) |
| `provider` | TEXT | `github` \| `gitlab` \| NULL (local) |
| `remote_id` | INTEGER | ID de la PR sur le provider distant |
| `source_url` | TEXT | URL de la PR sur le provider |
| `assignee` | TEXT | Utilisateur assigné |
| `created_at` | TEXT | Date de création (ISO 8601) |
| `updated_at` | TEXT | Date de dernière modification |

### 3.2 Table `labels`

| Colonne | Type | Description |
|---------|------|-------------|
| `id` | INTEGER PK | Identifiant auto-incrémenté |
| `name` | TEXT UNIQUE NOT NULL | Nom du label (ex: `bug`, `feature`) |
| `color` | TEXT | Couleur hex (ex: `#ff0000`) |

### 3.3 Table `task_labels`

| Colonne | Type | Description |
|---------|------|-------------|
| `task_id` | INTEGER FK | Référence vers `tasks.id` |
| `label_id` | INTEGER FK | Référence vers `labels.id` |

Contrainte : `UNIQUE(task_id, label_id)`

### 3.4 Table `reviewers`

| Colonne | Type | Description |
|---------|------|-------------|
| `id` | INTEGER PK | Identifiant auto-incrémenté |
| `task_id` | INTEGER FK | Référence vers `tasks.id` |
| `username` | TEXT NOT NULL | Nom d'utilisateur du reviewer |
| `status` | TEXT NOT NULL | `pending` \| `approved` \| `changes_requested` |
| `reviewed_at` | TEXT | Date de la review |

### 3.5 Table `comments`

| Colonne | Type | Description |
|---------|------|-------------|
| `id` | INTEGER PK | Identifiant auto-incrémenté |
| `task_id` | INTEGER FK | Référence vers `tasks.id` |
| `author` | TEXT NOT NULL | Auteur du commentaire |
| `body` | TEXT NOT NULL | Contenu du commentaire |
| `remote_id` | INTEGER | ID du commentaire distant |
| `created_at` | TEXT | Date de création |

---

## 4. Commandes CLI

### 4.1 Initialisation

```
devtodo init
```
- Crée le fichier `.devtodo.db` dans le répertoire courant
- Exécute les migrations SQL (création des tables)
- Détecte automatiquement le remote Git si disponible

### 4.2 Gestion des tâches

```
devtodo add <title> [options]
  -d, --description <text>    Description de la tâche
  -p, --priority <level>      Priorité : low|medium|high|critical
  -b, --branch <name>         Branche Git associée
  --base <branch>             Branche cible (défaut: main)
  -l, --label <name>          Labels (répétable)
  -a, --assignee <user>       Utilisateur assigné

devtodo list [options]
  -s, --status <status>       Filtrer par statut
  -l, --label <name>          Filtrer par label
  -p, --priority <level>      Filtrer par priorité
  -a, --assignee <user>       Filtrer par assigné
  --sort <field>              Trier par : created|updated|priority
  --limit <n>                 Nombre max de résultats

devtodo show <id>
  --comments                  Afficher les commentaires
  --json                      Sortie JSON

devtodo edit <id> [options]
  -t, --title <text>          Nouveau titre
  -d, --description <text>    Nouvelle description
  -p, --priority <level>      Nouvelle priorité
  -b, --branch <name>         Nouvelle branche
  -a, --assignee <user>       Nouvel assigné

devtodo status <id> <status>
  # status : draft | open | review | merged | closed

devtodo delete <id>
  --force                     Supprimer sans confirmation
```

### 4.3 Labels

```
devtodo label add <name> [--color <hex>]
devtodo label remove <name>
devtodo label list
devtodo label assign <task_id> <label_name>
devtodo label unassign <task_id> <label_name>
```

### 4.4 Reviews

```
devtodo review assign <task_id> <username>
devtodo review remove <task_id> <username>
devtodo review status <task_id> <username> <approved|changes_requested>
devtodo review list <task_id>
```

### 4.5 Synchronisation

```
devtodo sync [options]
  --provider <github|gitlab>  Sync avec un provider spécifique
  --dry-run                   Afficher les changements sans les appliquer

devtodo push <id>
  # Crée ou met à jour la PR sur le remote configuré

devtodo pull [options]
  --provider <github|gitlab>  Provider source
  --repo <owner/repo>         Repository distant
  --state <open|closed|all>   État des PRs à importer
```

### 4.6 Statistiques

```
devtodo stats [options]
  --period <7d|30d|90d|all>   Période d'analyse
```

Affiche :
- Nombre de tâches par statut
- Nombre de tâches par priorité
- Nombre de tâches par label
- Temps moyen entre création et merge
- Tâches les plus anciennes encore ouvertes
- Activité par semaine (sparkline)

### 4.7 Export

```
devtodo export <format> [options]
  # format : json | csv | markdown
  -o, --output <file>         Fichier de sortie (défaut: stdout)
  -s, --status <status>       Filtrer par statut
```

### 4.8 Configuration

```
devtodo config set <key> <value>
devtodo config get <key>
devtodo config list

# Clés de configuration :
# github.token      — Token d'accès personnel GitHub
# gitlab.token      — Token d'accès personnel GitLab
# gitlab.url        — URL de l'instance GitLab (défaut: gitlab.com)
# default.provider  — Provider par défaut (github|gitlab)
# default.base      — Branche cible par défaut (défaut: main)
# display.color     — Activer/désactiver les couleurs (true|false)
```

Stockage de la config : `~/.config/devtodo/config.toml` (XDG)

---

## 5. Intégration GitHub

### 5.1 API utilisée
- GitHub REST API v3
- Authentification : token personnel (`Authorization: Bearer <token>`)

### 5.2 Endpoints

| Action | Endpoint |
|--------|----------|
| Lister les PRs | `GET /repos/{owner}/{repo}/pulls` |
| Créer une PR | `POST /repos/{owner}/{repo}/pulls` |
| Mettre à jour une PR | `PATCH /repos/{owner}/{repo}/pulls/{number}` |
| Lister les reviews | `GET /repos/{owner}/{repo}/pulls/{number}/reviews` |
| Lister les commentaires | `GET /repos/{owner}/{repo}/pulls/{number}/comments` |
| Lister les labels | `GET /repos/{owner}/{repo}/labels` |
| Assigner des labels | `POST /repos/{owner}/{repo}/issues/{number}/labels` |

### 5.3 Mapping des statuts

| devtodo | GitHub |
|---------|--------|
| `draft` | PR avec `draft: true` |
| `open` | PR ouverte |
| `review` | PR avec review demandée |
| `merged` | PR mergée |
| `closed` | PR fermée |

---

## 6. Intégration GitLab

### 6.1 API utilisée
- GitLab REST API v4
- Authentification : token personnel (`PRIVATE-TOKEN: <token>`)
- Support des instances self-hosted (URL configurable)

### 6.2 Endpoints

| Action | Endpoint |
|--------|----------|
| Lister les MRs | `GET /projects/{id}/merge_requests` |
| Créer une MR | `POST /projects/{id}/merge_requests` |
| Mettre à jour une MR | `PUT /projects/{id}/merge_requests/{iid}` |
| Lister les approvals | `GET /projects/{id}/merge_requests/{iid}/approvals` |
| Lister les notes | `GET /projects/{id}/merge_requests/{iid}/notes` |
| Lister les labels | `GET /projects/{id}/labels` |

### 6.3 Mapping des statuts

| devtodo | GitLab |
|---------|--------|
| `draft` | MR avec préfixe `Draft:` |
| `open` | MR `opened` |
| `review` | MR avec approvals en attente |
| `merged` | MR `merged` |
| `closed` | MR `closed` |

---

## 7. Gestion des erreurs

Type d'erreur unifié via `thiserror` :

```rust
pub enum DevTodoError {
    Db(rusqlite::Error),          // Erreurs SQLite
    Api(reqwest::Error),          // Erreurs HTTP
    Config(String),               // Configuration manquante/invalide
    NotFound(String),             // Ressource introuvable
    InvalidStatus(String),        // Statut invalide
    Git(String),                  // Erreurs Git
    Io(std::io::Error),           // Erreurs I/O
    Serialization(serde_json::Error), // Erreurs de sérialisation
}
```

---

## 8. Workflow typique

```bash
# 1. Initialiser dans un repo Git
cd mon-projet
devtodo init

# 2. Configurer les tokens
devtodo config set github.token ghp_xxxxxxxxxxxx

# 3. Créer une tâche
devtodo add "Ajouter l'authentification JWT" \
  -d "Implémenter auth JWT avec refresh tokens" \
  -p high \
  -b feature/jwt-auth \
  -l feature -l security

# 4. Passer en review
devtodo status 1 review
devtodo review assign 1 alice

# 5. Pousser vers GitHub
devtodo push 1

# 6. Synchroniser les changements
devtodo sync

# 7. Merger
devtodo status 1 merged

# 8. Voir les stats
devtodo stats --period 30d

# 9. Exporter
devtodo export markdown -o TASKS.md
```

---

## 9. Contraintes et exigences

### 9.1 Performance
- L'init et les commandes locales doivent répondre en < 100ms
- Les commandes réseau doivent afficher un indicateur de progression
- La DB locale ne doit pas dépasser quelques Mo pour un usage normal

### 9.2 Compatibilité
- Linux, macOS, Windows
- Rust edition 2021, MSRV 1.75+

### 9.3 Sécurité
- Les tokens API ne sont jamais affichés en clair (masqués dans `config list`)
- Les tokens sont stockés dans le fichier config utilisateur avec permissions restrictives (600)
- Pas de stockage de tokens dans la DB du projet (qui peut être commitée)

### 9.4 UX
- Messages d'erreur clairs et actionnables
- Couleurs pour différencier les statuts et priorités
- Confirmation avant suppression (sauf `--force`)
- Autocomplétion shell (bash, zsh, fish) via `clap_complete`

---

## 10. Dépendances Cargo

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
clap_complete = "4"
rusqlite = { version = "0.31", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
directories = "5"
comfy-table = "7"
colored = "2"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
dialoguer = "0.11"           # Prompts interactifs (confirmation, sélection)
indicatif = "0.17"           # Barres de progression pour les syncs
```
