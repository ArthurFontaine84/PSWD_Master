# Installer PSWD Master et créer son premier coffre

Ce guide part d'une machine sans Rust et aboutit à un coffre utilisable en local avec l'application **PSWD Master** (§ 2).

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

1. **Nouveau coffre** : un nom (32 caractères au plus) et un mot de passe maître d'au moins 12 caractères mélangeant au moins 3 types parmi minuscules, majuscules, chiffres et symboles.
2. La **clé de récupération** s'affiche une seule fois : la recopier sur papier, cocher la case, puis **Continuer**. Elle permet de choisir un nouveau mot de passe en cas d'oubli (« Mot de passe oublié ? » à l'ouverture).
3. Le coffre est ouvert : **Nouvelle entrée** pour ajouter un mot de passe ou une note sécurisée, la recherche et le filtre par tag pour retrouver une entrée, **Copier** pour copier son mot de passe (effacé du presse-papiers après 20 s).

Le coffre se verrouille après 10 minutes sans action, ou avec **Verrouiller**. **Gérer le coffre** permet de changer le mot de passe maître, d'importer un fichier CSV (format : `docs/SPEC.md` § 8) ou de supprimer le coffre. Depuis la liste des coffres : **Sauvegarder** écrit les coffres cochés dans un fichier `.pswd` (§ 3), **Restaurer** rajoute ceux qu'on coche dans un tel fichier, **Réglages** choisit le thème.

## 3. Sauvegarder sur une clé USB

Brancher la clé, puis, depuis la liste des coffres : **Sauvegarder**, cocher les coffres voulus et confirmer. La fenêtre d'enregistrement du système propose un nom comme `pswd-sauvegarde_2026-10-23_14-30-00.pswd` (heure UTC) : choisir la clé comme emplacement.

Les coffres sont copiés, toujours chiffrés : aucun mot de passe n'est demandé et rien n'est lisible dans le fichier. C'est le même format sur ordinateur et sur téléphone ; il sert aussi à passer ses coffres d'un appareil à l'autre. Supprimer les sauvegardes les plus anciennes est à faire à la main.

Pour restaurer, sur cet appareil ou un autre : **Restaurer**, choisir le fichier `.pswd`, cocher les coffres voulus et confirmer. Chaque coffre est ajouté comme nouveau coffre ; si le nom est déjà pris, il devient `perso-sauvegarde-2026-10-23` : un coffre existant n'est jamais remplacé. Il s'ouvre avec le mot de passe maître en vigueur au moment de la sauvegarde.

## 4. Où sont les fichiers

| Élément | Windows | Linux / macOS |
|---|---|---|
| Configuration | `%APPDATA%\pswd\config.toml` | `~/.config/pswd/config.toml` |
| Coffres | `%APPDATA%\pswd\vaults\<identifiant>\` | `~/.local/share/pswd/vaults/<identifiant>/` |

Chaque dossier de coffre contient `vault.enc`, le coffre chiffré. Dans `config.toml`, on peut régler `auto_lock_minutes` (0 = jamais) et `clipboard_clear_seconds` ; `theme` est réglé par PSWD Master. La version de développement (`npm run tauri dev`) utilise des dossiers à part, `pswd-dev`, et ne touche jamais les coffres de la version installée.

**Venant d'une version précédente** : retirer les lignes `url = …` et `default_vault = …` de `config.toml` (sinon : `configuration invalide`). Les dossiers `repo.git` et les jetons `pswd` du trousseau ne servent plus et peuvent être supprimés.

## 5. Serveur local pour le développement

> ⚠ Réservé au développement : le serveur tourne au premier plan, sur la machine de développement, et n'écoute que sur elle. L'installation réelle (paquet `.deb`, service `systemd`) est décrite dans `docs/SERVEUR.md`.

Depuis le dossier du projet (`--data-dir` est obligatoire hors Linux : le dossier par défaut est `/var/lib/pswd-server`) :

```sh
mkdir C:\pswd-test-server
cargo run -p pswd-server -- --data-dir C:\pswd-test-server reset-admin   # crée la base et le certificat, affiche une clé administrateur
cargo run -p pswd-server -- --data-dir C:\pswd-test-server serve --listen 127.0.0.1:8443   # Ctrl+C pour l'arrêter
```

La clé administrateur ne sert qu'une fois et expire dans 7 jours ; `reset-admin` en donne une nouvelle. Dans PSWD Master, écran **Serveur** : adresse `127.0.0.1` (le port 8443 est ajouté), puis la clé. À la connexion, tous les coffres de l'appareil sont publiés sur ce serveur : faire d'abord une sauvegarde (§ 3) si des coffres réels s'y trouvent. Pour repartir de zéro, arrêter le serveur et vider le dossier.

## 6. En cas de problème

| Message | Cause et solution |
|---|---|
| « Un coffre nommé … existe déjà. » | Ce nom est pris sur cet appareil : en choisir un autre. |
| « Mot de passe maître incorrect. » | Réessayer, ou « Mot de passe oublié ? » avec la clé de récupération. |
| ``configuration invalide : unknown field `url` `` (ou `default_vault`) | Configuration d'une version précédente : voir § 4. |

**Supprimer un coffre de cet appareil** : **Gérer le coffre** → **Supprimer ce coffre**, puis retaper le nom pour confirmer. Faire d'abord une sauvegarde (§ 3) pour pouvoir le restaurer.
