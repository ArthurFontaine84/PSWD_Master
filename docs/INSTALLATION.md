# Installer pswd et créer son premier coffre

Ce guide part d'une machine sans Rust et aboutit à un coffre utilisable en local.

## 1. Prérequis

| Système | À installer |
|---|---|
| Windows | [Rust](https://rustup.rs) (`rustup-init.exe`) et les **Build Tools de Visual Studio**, charge de travail « Développement Desktop en C++ » : Rust s'en sert pour assembler le programme. |
| Linux | Rust (`curl https://sh.rustup.rs -sSf \| sh`), puis `build-essential` (Debian/Ubuntu). |
| macOS | Rust, puis les outils en ligne de commande Xcode (`xcode-select --install`). |

Vérifier l'installation de Rust :

```sh
cargo --version
```

## 2. Compiler en release

Depuis le dossier du projet :

```sh
cargo build --release -p pswd-cli
```

La première compilation prend quelques minutes. Le programme obtenu est un fichier unique :

- Windows : `target\release\pswd.exe`
- Linux / macOS : `target/release/pswd`

Pour pouvoir taper `pswd` depuis n'importe quel dossier, l'installer dans `~/.cargo/bin` (déjà dans le `PATH` après l'installation de Rust) :

```sh
cargo install --path crates/pswd-cli
pswd --version
```

Pour mettre à jour après un `git pull`, relancer la même commande `cargo install`.

## 3. Créer le coffre

```sh
pswd init perso
```

`perso` est le nom du coffre : lettres, chiffres, `-` et `_`. Le programme demande un **nouveau mot de passe maître**, saisi deux fois et masqué. Il doit avoir au moins 12 caractères et mélanger au moins 3 types parmi minuscules, majuscules, chiffres et symboles ; il doit aussi être unique : c'est la seule chose à retenir.

Le coffre est créé et chiffré. La **clé de récupération** s'affiche alors, **une seule fois** :

```
    000G-40R4-0M30-E209-185G-R38E-1W81-24GK-2GAH-C5RR-34D1-P70X-3RFG
```

Recopier cette clé sur papier et la ranger en lieu sûr, à l'écart du mot de passe. Elle permet de choisir un nouveau mot de passe maître en cas d'oubli (`pswd recover perso`).

> ⚠ Mot de passe maître **et** clé de récupération perdus = coffre perdu. Personne ne peut le déchiffrer.

## 4. Utiliser le coffre

```sh
pswd open perso
```

Après le mot de passe maître, un shell s'ouvre (`help` pour l'aide) :

| Commande | Effet |
|---|---|
| `add` / `add --generate` | Ajoute une entrée, champ par champ ; `--generate` crée un mot de passe de 24 caractères. |
| `list [--tag t]` | Liste les entrées. |
| `search <texte>` | Cherche dans les titres, identifiants, URLs et tags. |
| `show <entrée> [--show]` | Affiche une entrée ; mot de passe masqué sauf avec `--show`. |
| `copy <entrée>` | Copie le mot de passe ; effacé du presse-papiers après 20 s. |
| `edit <entrée>` | Modifie une entrée : réponse vide = inchangé, `-` = effacer. |
| `rm <entrée>` | Supprime une entrée (confirmation demandée). |
| `history <entrée>` | Versions précédentes. |
| `tags` | Tags et nombre d'entrées. |
| `gen [--length N] [--no-symbols]` | Génère un mot de passe. |
| `quit` / `lock` | Ferme le coffre. |

Une entrée se désigne par son titre (`show github`, `show "code wi-fi"`) ou par le début de son identifiant affiché par `list`. Chaque modification est enregistrée immédiatement. Sans commande pendant 10 minutes, le coffre se verrouille.

Commandes ponctuelles :

```sh
pswd passwd perso                # changer le mot de passe maître
pswd recover perso               # nouveau mot de passe grâce à la clé de récupération
pswd import perso fichier.csv    # importer (format : docs/SPEC.md § 8)
pswd delete essai                # supprimer un coffre (nom à retaper)
```

## 5. Sauvegarder sur une clé USB

Brancher la clé (ici `E:`), puis :

```sh
pswd backup E:\                  # tous les coffres ; --vault perso pour un seul
```

Les coffres sont copiés, toujours chiffrés, dans un nouveau fichier : `E:\pswd-sauvegarde_2026-10-23_14-30-00.pswd` (heure UTC). C'est le même format que l'application PSWD Master, sur ordinateur comme sur téléphone. Les sauvegardes précédentes ne sont jamais modifiées ; supprimer les plus anciennes est à faire à la main.

Pour restaurer, sur cette machine ou une autre :

```sh
pswd restore E:\pswd-sauvegarde_2026-10-23_14-30-00.pswd          # liste les coffres et demande lequel
pswd restore E:\pswd-sauvegarde_2026-10-23_14-30-00.pswd perso    # directement
```

Le coffre est ajouté comme nouveau coffre. Si le nom est déjà pris, il devient `perso-sauvegarde-2026-10-23` : le coffre existant n'est jamais remplacé. Il s'ouvre avec le mot de passe maître en vigueur au moment de la sauvegarde.

## 6. Où sont les fichiers

| Élément | Windows | Linux / macOS |
|---|---|---|
| Configuration | `%APPDATA%\pswd\config.toml` | `~/.config/pswd/config.toml` |
| Coffres | `%APPDATA%\pswd\vaults\<nom>\` | `~/.local/share/pswd/vaults/<nom>/` |

Chaque dossier de coffre contient `vault.enc`, le coffre chiffré. Dans `config.toml`, on peut régler `auto_lock_minutes` (0 = jamais) et `clipboard_clear_seconds`.

**Venant d'une version précédente** : retirer les lignes `url = …` et `default_vault = …` de `config.toml` (sinon : `configuration invalide`). Les dossiers `repo.git` et les jetons `pswd` du trousseau ne servent plus et peuvent être supprimés.

## 7. En cas de problème

| Message | Cause et solution |
|---|---|
| `un coffre nommé … existe déjà` | Ce nom est pris sur cette machine : en choisir un autre. |
| `mot de passe maître incorrect` | Réessayer, ou `pswd recover <nom>` avec la clé de récupération. |
| ``configuration invalide : unknown field `url` `` (ou `default_vault`) | Configuration d'une version précédente : voir § 6. |

**Supprimer un coffre de cette machine** : `pswd delete <nom>`, puis retaper le nom pour confirmer. Faire d'abord une sauvegarde (§ 5) pour pouvoir le restaurer.
