# Compiler PSWD Master pour macOS, iPhone et iPad

Procédure d'installation sur un Mac, pour construire l'application de `crates/pswd-gui` (Tauri 2).

- **macOS** et **iOS** sont deux formats différents : `.app` / `.dmg` d'un côté, `.ipa` de l'autre.
- **iPad** : pas de format à part. La même application iOS tourne sur iPhone et iPad (§ 5).
- macOS se contente des outils en ligne de commande ; iOS exige **Xcode complet** et un **compte Apple**.

## 1. Base commune (une seule fois)

1. **Xcode**, depuis l'App Store (15 à 40 Go avec la plateforme iOS). L'ouvrir une première fois, puis :
   ```sh
   sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
   sudo xcodebuild -license accept
   xcodebuild -runFirstLaunch
   ```
   Dans Xcode → *Settings* → *Components*, vérifier que **iOS** est installé (avec un simulateur si besoin).
2. **Homebrew** : la commande d'installation est sur [brew.sh](https://brew.sh).
3. **Rust** :
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
   Rouvrir le terminal, puis `cargo --version`.
4. **Node.js** 22.12 ou plus récent : `brew install node`, puis `node --version`.
5. **Le projet** (`git` est fourni avec Xcode) :
   ```sh
   git clone <url-du-dépôt> && cd PSWD_Master/crates/pswd-gui
   npm ci
   ```

## 2. macOS

```sh
npm run tauri build
```

Résultat dans `target/release/bundle/macos/PSWD Master.app` et `target/release/bundle/dmg/` (racine du dépôt). Construit sur le Mac lui-même, il s'ouvre sans avertissement de Gatekeeper.

Facultatif, une version « universelle » (Mac Intel et Apple Silicon) :

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

## 3. iOS : préparation

1. **Cibles Rust** (vrai téléphone, puis simulateurs) :
   ```sh
   rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
   ```
2. **CocoaPods**, demandé par les prérequis iOS de Tauri : `brew install cocoapods`.
3. **Compte Apple dans Xcode** : *Settings* → *Accounts* → « + » → identifiant Apple, puis *Manage Certificates* → « + » → **Apple Development**.
   - **Compte gratuit** (« Personal Team ») : l'application installée sur le téléphone expire au bout de **7 jours** ; il faut la réinstaller depuis le Mac.
   - **Programme développeur** (99 €/an) : validité d'un an, accès à TestFlight et à l'App Store. Le compte gratuit suffit pour tester.
4. **Générer le projet Xcode**, depuis `crates/pswd-gui` :
   ```sh
   npm run tauri ios init
   npx tauri icon src-tauri/app-icon/icon.json   # icônes iOS, à relancer après chaque `ios init`
   ```
   `ios init` détecte normalement l'équipe de signature. Sinon : `npm run tauri ios dev -- --open`, puis dans Xcode *Signing & Capabilities* → *Team*.

## 4. iOS : lancer l'application

**Simulateur** :

```sh
npm run tauri ios dev   # propose la liste des simulateurs
```

**iPhone réel** :

1. Le brancher en USB et répondre « Faire confiance » à cet ordinateur.
2. Activer le **mode développeur** : *Réglages → Confidentialité et sécurité → Mode développeur* (l'option n'apparaît qu'une fois que Xcode a vu l'appareil ; le téléphone redémarre).
3. `npm run tauri ios dev` et choisir l'iPhone. En mode dev, l'interface est servie par le Mac (`TAURI_DEV_HOST`, géré dans `vite.config.ts`) : Mac et iPhone sur le même Wi-Fi, et accepter l'accès au réseau local.
4. Au premier lancement, autoriser le développeur : *Réglages → Général → VPN et gestion de l'appareil* → le compte → « Faire confiance ».

**Application autonome** (sans le Mac) :

```sh
npm run tauri ios build -- --export-method debugging
```

Le `.ipa` est rangé sous `src-tauri/gen/apple/build/`. Installation : Xcode → *Window → Devices and Simulators* → l'iPhone → « + » sous *Installed Apps*.

## 5. iPad

Rien de plus à construire : dans Xcode, cible de l'application → *General* → *Supported Destinations*, vérifier que **iPad** est présent (l'ajouter sinon). Le même `.ipa` s'installe alors sur l'iPad de la même façon. L'interface gère déjà les grands écrans.

## 6. Reste à faire pour iOS

Le build devrait aboutir, mais la partie propre à iOS n'est pas encore codée (SPEC § 12) ; en attendant, **la copie dans le presse-papiers renvoie une erreur sur iOS** :

- plugin Swift du presse-papiers, dans une petite crate dédiée (à cause de `forbid(unsafe_code)`) ;
- écran neutre quand l'application passe en arrière-plan ;
- accès « security-scoped » à refermer (sauvegarde et restauration par fichier) ;
- essai de l'ouverture des adresses dans le navigateur (plugin `opener`).

`src-tauri/gen/apple` sera vraisemblablement versionné, comme `gen/android`, puisque l'écran neutre s'ajoute dans ce projet Xcode.
