# Installer PSWD Master et pswd, créer son premier coffre

Ce guide part d'une machine sans Rust et aboutit à un coffre utilisable en local, avec l'application graphique **PSWD Master** (§ 2), la ligne de commande **`pswd`** (§ 3 à 6), ou les deux : sur un ordinateur, elles voient les mêmes coffres.

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

Pour l'application PSWD Master, il faut aussi [Node.js](https://nodejs.org) 22.12 ou plus récent, et sous Linux les bibliothèques de la vue web :

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## 2. Application PSWD Master

### Construire l'installateur

Depuis le dossier du projet :

```sh
cd crates/pswd-gui
npm ci                   # dépendances de l'interface, une seule fois
npm run tauri build      # quelques minutes la première fois
```

L'installateur est rangé dans `target/release/bundle/` (à la racine du projet) :

| Système | Fichier | Installation |
|---|---|---|
| Windows | `nsis/PSWD Master_<version>_x64-setup.exe` | Double-clic. S'installe pour l'utilisateur courant, sans droits d'administrateur. La vue web (WebView2) est déjà présente sous Windows 10 et 11 ; sinon l'installateur la télécharge. |
| Linux | `deb/PSWD Master_<version>_amd64.deb`, `appimage/PSWD Master_<version>_amd64.AppImage` | `sudo apt install ./<fichier>.deb`, ou rendre l'AppImage exécutable et la lancer. |
| macOS | `dmg/PSWD Master_<version>_<arch>.dmg` | Ouvrir, puis glisser l'application dans « Applications ». |

L'installateur n'est pas signé : Windows SmartScreen et macOS Gatekeeper peuvent afficher un avertissement à la première ouverture (« Informations complémentaires » → « Exécuter quand même » sous Windows ; clic droit → « Ouvrir » sous macOS).

Pour mettre à jour : `git pull`, reconstruire, puis relancer le nouvel installateur ; les coffres ne sont pas touchés.

### Construire l'APK Android

Se fait depuis un ordinateur (Windows, Linux ou macOS). Prérequis, en plus de ceux du § 1 :

- [Android Studio](https://developer.android.com/studio), puis dans *SDK Manager* : un SDK Android, *NDK (Side by side)*, *Android SDK Build-Tools* et *Android SDK Command-line Tools* ;
- les variables d'environnement `JAVA_HOME` (le JDK fourni, `…\Android Studio\jbr`), `ANDROID_HOME` (le SDK, `%LOCALAPPDATA%\Android\Sdk` sous Windows) et `NDK_HOME` (`$ANDROID_HOME/ndk/<version>`) ;
- les cibles Rust :

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Construire, depuis `crates/pswd-gui` :

```sh
npm ci                                                 # une seule fois
npm run tauri android build -- --apk --target aarch64  # téléphones 64 bits (presque tous)
```

L'APK est signé pendant le build si le fichier `src-tauri/gen/android/keystore.properties` existe (ignoré par git). Sans lui, l'APK sort non signé (`app-universal-release-unsigned.apk`) et Android refuse de l'installer. Pour le créer, **une seule fois** : générer une clé hors du projet (`keytool` est dans `$JAVA_HOME/bin`) …

```sh
keytool -genkeypair -keystore ~/pswd-master.jks -storetype PKCS12 -keyalg RSA -keysize 4096 -validity 10000 -alias pswd-master
```

… puis écrire `keystore.properties` (chemin avec des `/`, même sous Windows) :

```properties
storeFile=C:/Users/<nom>/pswd-master.jks
storePassword=<mot de passe de la clé>
keyAlias=pswd-master
keyPassword=<mot de passe de la clé>
```

L'APK signé est `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`. L'installer : téléphone branché en USB avec le débogage USB activé, `adb install -r <fichier>.apk` (`adb` est dans `$ANDROID_HOME/platform-tools/`) ; ou copier le fichier sur le téléphone et l'ouvrir, en autorisant l'installation d'applications inconnues.

> ⚠ Les mises à jour doivent être signées avec **la même clé** : sinon Android refuse de remplacer l'application (c'est le cas en passant d'un build `--debug` à un build release), et la désinstaller efface ses coffres. Sauvegarder `pswd-master.jks` et `keystore.properties` hors de l'ordinateur, et faire une sauvegarde `.pswd` des coffres avant toute désinstallation.

### Premier coffre

1. **Nouveau coffre** : un nom (lettres, chiffres, `-` et `_`) et un mot de passe maître d'au moins 12 caractères mélangeant au moins 3 types parmi minuscules, majuscules, chiffres et symboles.
2. La **clé de récupération** s'affiche une seule fois : la recopier sur papier, cocher la case, puis **Continuer**. Elle permet de choisir un nouveau mot de passe en cas d'oubli (« Mot de passe oublié ? » à l'ouverture).
3. Le coffre est ouvert : **Nouvelle entrée** pour ajouter un identifiant ou une note, la recherche et le filtre par tag pour retrouver une entrée, **Copier** pour copier son mot de passe (effacé du presse-papiers après 20 s).

Le coffre se verrouille après 10 minutes sans action, ou avec **Verrouiller**. **Gérer le coffre** permet de changer le mot de passe maître, d'importer un fichier CSV (format : `docs/SPEC.md` § 8) ou de supprimer le coffre. Depuis la liste des coffres : **Sauvegarder** écrit tous les coffres dans un fichier `.pswd` (même format qu'au § 6), **Restaurer une sauvegarde** en rajoute un depuis ce fichier, **Réglages** choisit le thème.

## 3. Installer la ligne de commande

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

## 4. Créer le coffre

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

## 5. Utiliser le coffre

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

## 6. Sauvegarder sur une clé USB

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

## 7. Où sont les fichiers

| Élément | Windows | Linux / macOS |
|---|---|---|
| Configuration | `%APPDATA%\pswd\config.toml` | `~/.config/pswd/config.toml` |
| Coffres | `%APPDATA%\pswd\vaults\<nom>\` | `~/.local/share/pswd/vaults/<nom>/` |

Ces emplacements sont communs à `pswd` et à PSWD Master sur un ordinateur ; les deux peuvent modifier le même coffre en même temps. Chaque dossier de coffre contient `vault.enc`, le coffre chiffré. Dans `config.toml`, on peut régler `auto_lock_minutes` (0 = jamais) et `clipboard_clear_seconds`.

**Venant d'une version précédente** : retirer les lignes `url = …` et `default_vault = …` de `config.toml` (sinon : `configuration invalide`). Les dossiers `repo.git` et les jetons `pswd` du trousseau ne servent plus et peuvent être supprimés.

## 8. En cas de problème

| Message | Cause et solution |
|---|---|
| `un coffre nommé … existe déjà` | Ce nom est pris sur cette machine : en choisir un autre. |
| `mot de passe maître incorrect` | Réessayer, ou `pswd recover <nom>` avec la clé de récupération. |
| ``configuration invalide : unknown field `url` `` (ou `default_vault`) | Configuration d'une version précédente : voir § 7. |

**Supprimer un coffre de cette machine** : `pswd delete <nom>`, puis retaper le nom pour confirmer. Faire d'abord une sauvegarde (§ 6) pour pouvoir le restaurer.
