# Développement

Règles de qualité du projet et commandes de vérification. La conception de référence est dans [`SPEC.md`](SPEC.md) : toute décision qui la change y est reportée.

## Règles

**Tests** : tout code est livré avec ses tests, dans le même commit.

| Partie | Exigence |
|---|---|
| `pswd-core` | **100 % des lignes et des branches** (`cargo llvm-cov`, seuil bloquant) et **aucun mutant survivant** (`cargo mutants`). Seule exception : une erreur de bibliothèque impossible à atteindre, isolée dans une petite fonction dédiée marquée `#[cfg_attr(coverage_nightly, coverage(off))]`, avec un commentaire qui le justifie. |
| `pswd-infra`, `pswd-app`, `pswd-server`, backend de `pswd-gui` | 100 % visé. Toute exclusion est explicite, commentée et justifiée ligne par ligne. |
| Interface (TypeScript) | Vitest, 100 % des lignes pour la logique `.ts` (seuil bloquant) ; composants Svelte testés par leur comportement (Testing Library). |
| Code natif (Kotlin, Swift) | Réduit au minimum, testé à la main (SPEC §12). |

- Tests de propriétés (`proptest`) : fusion (commutative, associative, idempotente), aller-retour de la crypto, aller-retour de l'import CSV.
- Fuzzing (`cargo fuzz`) des parseurs : `vault.enc`, payload JSON, sauvegardes `.pswd`, CSV.
- Synchro testée avec un distant en mémoire, et en intégration contre un `pswd-server` local.
- Un test vérifie un comportement : pas de test sans assertion significative.

**Code**

- `#![forbid(unsafe_code)]` dans `pswd-core`, `pswd-app`, `pswd-gui` et `pswd-server`. Pas d'`unwrap()` ni d'`expect()` hors des tests (refusés par Clippy).
- Secrets dans des types effacés à la libération (`Zeroizing`, `Secret` de `pswd-core`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- Interface : aucun secret côté JavaScript sans action explicite de l'utilisateur ; aucun texte en dur hors de `src/lib/i18n/<langue>.ts` (SPEC §9, §12).
- Aucune commande n'écrase un coffre existant.
- Points d'extension futurs marqués `// EXTENSION(<nom>): …` (liste dans SPEC §10).

## Vérifications avant un commit

À la racine :

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
cargo nextest run --workspace
cargo +nightly llvm-cov nextest --workspace --branch --no-rustc-wrapper --fail-under-lines 100   # --branch exige nightly
cargo deny check
```

`--no-rustc-wrapper` est indispensable sous Windows : la commande de compilation de `pswd-gui` dépasse la longueur maximale, cargo la passe à `rustc` dans un fichier `@argfile`, et le wrapper de `cargo-llvm-cov` n'instrumente alors pas la crate (rapport vide, sans erreur). Après chaque campagne, vérifier que le rapport liste bien les fichiers de chaque crate.

Dans `crates/pswd-gui` :

```sh
npm run check        # svelte-check + ESLint, aucun avertissement
npm test             # Vitest avec couverture
npm audit
```

## Tests de mutation

```sh
cargo mutants -p pswd-core
```

Une douzaine de minutes : lancé à la fin de chaque grande étape de la feuille de route (SPEC §2). Les mutants équivalents écartés sont listés et justifiés dans `.cargo/mutants.toml`.

## Fuzzing

Quand un parseur change, fuzzer chaque cible quelques minutes (dossier `fuzz/`, nightly) :

```sh
cargo +nightly fuzz run vault_file -- -max_total_time=300
cargo +nightly fuzz run payload -- -max_total_time=300
mkdir -p corpus/backup_file && cargo +nightly fuzz run backup_file corpus/backup_file seeds/backup_file -- -max_total_time=300
mkdir -p corpus/csv && cargo +nightly fuzz run csv corpus/csv seeds/csv -- -dict=csv.dict -max_total_time=300
```

Le CSV part d'un corpus de départ (`fuzz/seeds/csv`) et d'un dictionnaire (`fuzz/csv.dict`) : sans eux, le fuzzer ne trouve jamais l'en-tête. De même, `fuzz/seeds/backup_file` fournit des `.pswd` valides : le fuzzer ne fabriquerait pas seul un `vault.enc` correct à l'intérieur. Après chaque campagne, vérifier que le corpus atteint bien le code visé.

Sous Windows, AddressSanitizer exige que le dossier de `clang_rt.asan_dynamic-x86_64.dll` (Visual Studio, `VC/Tools/MSVC/<version>/bin/Hostx64/x64`) soit dans le `PATH`.

## Builds

- Bureau et Android : [`INSTALLATION.md`](INSTALLATION.md).
- Serveur (paquet `.deb`, Docker) : [`SERVEUR.md`](SERVEUR.md).
