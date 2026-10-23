# Installer pswd et créer son premier coffre

Ce guide part d'une machine sans Rust et aboutit à un coffre synchronisé sur GitHub.

## 1. Prérequis

| Système | À installer |
|---|---|
| Windows | [Rust](https://rustup.rs) (`rustup-init.exe`) et les **Build Tools de Visual Studio**, charge de travail « Développement Desktop en C++ » : libgit2 est compilée en C. |
| Linux | Rust (`curl https://sh.rustup.rs -sSf \| sh`), puis `build-essential`, `pkg-config`, `libssl-dev` (Debian/Ubuntu) ; un trousseau Secret Service actif (GNOME Keyring ou KWallet). |
| macOS | Rust, puis les outils en ligne de commande Xcode (`xcode-select --install`). |

Git n'est **pas** nécessaire au fonctionnement de pswd : la bibliothèque Git est intégrée au programme. Il sert seulement à cloner les sources, et à créer un dépôt de test local (§ 6).

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

## 3. Préparer le dépôt GitHub

Un coffre = un dépôt Git **privé**.

1. Sur GitHub : **New repository** → un nom (par exemple `coffre-perso`), visibilité **Private**. Cocher ou non « Add a README » : les deux fonctionnent.
2. Créer un jeton d'accès limité à ce seul dépôt. Attention : il ne s'agit pas des réglages du dépôt, mais de ceux **du compte** — avatar en haut à droite → **Settings**, puis tout en bas de la barre latérale → **Developer settings → Personal access tokens → Fine-grained tokens → Generate new token** (lien direct : <https://github.com/settings/personal-access-tokens>) :
   - *Repository access* : **Only select repositories** → le dépôt du coffre ;
   - *Permissions → Repository permissions → Contents* : **Read and write** (*Metadata* passe automatiquement en lecture) ;
   - une date d'expiration : à son échéance, il faudra en créer un autre.
3. Copier le jeton (`github_pat_…`) : GitHub ne l'affiche qu'une fois. Il sera rangé dans le trousseau du système, jamais en clair sur le disque.
4. Noter l'URL HTTPS du dépôt : `https://github.com/<utilisateur>/coffre-perso.git`.

## 4. Créer le coffre

```sh
pswd init perso
```

`perso` est le nom local du coffre : lettres, chiffres, `-` et `_`. Le programme demande :

| Question | Réponse |
|---|---|
| URL du dépôt distant | L'URL du § 3. |
| Jeton d'accès au dépôt | Le jeton `github_pat_…` (saisie masquée). |
| Nouveau mot de passe maître | Saisi deux fois, masqué. Long et unique : c'est la seule chose à retenir. |

Le coffre est créé, chiffré, puis publié sur GitHub. La **clé de récupération** s'affiche alors, **une seule fois** :

```
    000G-40R4-0M30-E209-185G-R38E-1W81-24GK-2GAH-C5RR-34D1-P70X-3RFG
```

Recopier cette clé sur papier et la ranger en lieu sûr, à l'écart du mot de passe. Elle permet de choisir un nouveau mot de passe maître en cas d'oubli (`pswd recover`).

> ⚠ Mot de passe maître **et** clé de récupération perdus = coffre perdu. Personne, pas même GitHub, ne peut le déchiffrer.

Le premier coffre créé devient le coffre par défaut : les commandes sans nom l'utilisent.

## 5. Utiliser le coffre

```sh
pswd open
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
| `sync` | Synchronise avec GitHub. |
| `quit` / `lock` | Ferme le coffre. |

Une entrée se désigne par son titre (`show github`, `show "code wi-fi"`) ou par le début de son identifiant affiché par `list`. Chaque modification est enregistrée localement puis publiée. Hors ligne, le coffre reste utilisable, et les modifications partent à la synchronisation suivante. Sans commande pendant 10 minutes, le coffre se verrouille.

Commandes ponctuelles :

```sh
pswd sync                        # synchroniser sans ouvrir le shell
pswd passwd                      # changer le mot de passe maître
pswd recover                     # nouveau mot de passe grâce à la clé de récupération
pswd import fichier.csv          # importer (format : docs/SPEC.md § 8)
pswd export sauvegarde.csv       # exporter en clair (double confirmation)
```

## 6. Essayer sans GitHub

Pour un premier essai sans risque, un dépôt local tient lieu de GitHub :

```sh
git init --bare C:\coffres\essai.git      # Linux / macOS : git init --bare ~/coffres/essai.git
pswd init essai
```

À la question de l'URL, donner le chemin `C:\coffres\essai.git`. Laisser le jeton vide (touche Entrée). Le coffre `essai` s'ouvre ensuite avec `pswd open essai`.

## 7. Une deuxième machine

Sur l'autre ordinateur, après les §§ 1-2 :

```sh
pswd join perso https://github.com/<utilisateur>/coffre-perso.git
```

Saisir le jeton (le même, ou un second jeton pour ce dépôt), puis le mot de passe maître. Les deux machines se synchronisent à chaque ouverture et à chaque modification. Un changement de mot de passe maître fait sur l'une est adopté par l'autre à sa synchronisation suivante.

## 8. Où sont les fichiers

| Élément | Windows | Linux / macOS |
|---|---|---|
| Configuration | `%APPDATA%\pswd\config.toml` | `~/.config/pswd/config.toml` |
| Coffres | `%APPDATA%\pswd\vaults\<nom>\` | `~/.local/share/pswd/vaults/<nom>/` |
| Jeton | Gestionnaire d'identifiants Windows, service `pswd` | Trousseau du système, service `pswd` |

Chaque dossier de coffre contient `vault.enc` (le coffre chiffré) et `repo.git` (le cache du dépôt distant). Dans `config.toml`, on peut régler `auto_lock_minutes` (0 = jamais) et `clipboard_clear_seconds`.

## 9. En cas de problème

| Message | Cause et solution |
|---|---|
| `le dépôt distant contient déjà un coffre : utiliser join` | Le dépôt a déjà servi : `pswd join <nom> <url>`, ou un nouveau dépôt vide pour `init`. |
| `un coffre nommé … existe déjà` | Ce nom est pris sur cette machine : en choisir un autre. |
| `⚠ Hors ligne (…)` | GitHub injoignable, ou jeton refusé ou expiré. Le coffre local reste utilisable ; créer un nouveau jeton si besoin. |
| `mot de passe maître incorrect` | Réessayer, ou `pswd recover` avec la clé de récupération. |
| `⚠ Fichier distant refusé` | Le fichier sur GitHub est altéré ou appartient à un autre coffre : il n'est jamais écrasé. Vérifier l'URL du dépôt. |

**Supprimer un coffre de cette machine** (aucune commande pour l'instant) : supprimer son dossier, retirer sa section `[vaults.<nom>]` de `config.toml`, puis l'identifiant `pswd` / `<nom>` du trousseau. Le dépôt GitHub, lui, n'est pas touché.
