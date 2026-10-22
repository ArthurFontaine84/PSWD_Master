# SPEC — pswd

Référence de conception. Toute décision qui change ce document doit y être reportée.

## 1. Objectif

Gestionnaire de mots de passe personnel, local-first, pour remplacer un service tiers (NordPass).
Le coffre est chiffré de bout en bout : aucun serveur (GitHub, puis serveur maison) ne voit jamais de données en clair ni de clé.

## 2. Périmètre

**v1**
- CLI `pswd` sous Windows et Linux.
- Plusieurs coffres indépendants (un mot de passe maître, une clé, un dépôt Git par coffre).
- Accès hors ligne permanent ; synchro Git avec fusion entrée par entrée.
- Entrées `Login` et `Note`, tags, historique des versions.
- Générateur de mots de passe, copie sécurisée dans le presse-papiers.
- Changement de mot de passe maître, clé de récupération.
- Import / export CSV (format §8).

**Hors v1 (le terrain doit être préparé, cf. §10)**
- Application mobile (Android / iOS).
- Fichier-clé comme second facteur.
- Rotation de la clé du coffre.
- Serveur Git maison (remplace GitHub, aucun changement de code attendu).
- Adaptateur « dossier synchronisé » sans Git.

**Exclu**
- Coffres partagés entre plusieurs personnes (chacun a son coffre).
- Codes TOTP, champs personnalisés, extension navigateur / autofill.

## 3. Architecture

Workspace Cargo, architecture hexagonale :

| Crate | Contenu | Contraintes |
|---|---|---|
| `pswd-core` | Modèles, fusion, format de fichier, crypto, générateur, CSV, ports (traits) | Aucune E/S, aucun appel OS, `#![forbid(unsafe_code)]`, compilable pour mobile |
| `pswd-infra` | Adaptateurs desktop : `LocalStore` fichier, `RemoteStore` Git, trousseau, `SecureClipboard`, config | |
| `pswd-cli` | Binaire `pswd`, shell interactif, verrouillage automatique | `#![forbid(unsafe_code)]` |

Futur : `pswd-mobile` (bindings UniFFI au-dessus de `pswd-core`, adaptateurs propres).

**Ports (définis dans `pswd-core`)**
- `LocalStore` : `load() -> bytes`, `save_atomic(bytes)` (fichier temporaire + renommage).
- `RemoteStore` : `fetch() -> (bytes, version)`, `push(bytes, expected_version)` (échoue si le distant a bougé), `is_reachable()`.
- `SecretStore` : lire / écrire le jeton d'accès distant d'un coffre.
- `SecureClipboard` : `copy_secret(value, ttl)`.

Avec Git, `LocalStore` et `RemoteStore` visent le même clone local ; ils restent distincts pour permettre un transport sans Git.

La crypto n'est **pas** un port : déterministe, sans E/S, identique partout.

**Dépendances prévues** : `argon2`, `chacha20poly1305`, `hkdf` + `sha2`, `getrandom`, `rand` (générateur de mots de passe), `zeroize` / `secrecy`, `serde` + `serde_json`, `csv`, `uuid`, `keyring`, `directories`, `toml`, `clap`. Librairie Git embarquée (`git2` ou `gix`, à trancher au démarrage ; critères : push HTTPS fiable et compilation Android). Jamais le binaire `git` du système.

## 4. Modèle de données

```rust
enum EntryKind { Login, Note }

struct Entry {
    id: Uuid,                    // v4, immuable, clé de fusion
    kind: EntryKind,
    title: String,
    username: Option<String>,
    password: Option<Secret>,
    urls: Vec<String>,
    notes: Option<String>,
    tags: Tags,                  // normalisés : trim, minuscules, sans vide, uniques, triés
    created_at: Timestamp,       // UTC, millisecondes
    updated_at: Timestamp,
    deleted: bool,               // marqueur de suppression (tombstone)
    deleted_at: Option<Timestamp>, // date de la dernière suppression (conservée si l'entrée revit)
    history: Vec<EntrySnapshot>, // versions précédentes, 10 max, plus récentes d'abord
}

struct EntrySnapshot { content: EntryContent, updated_at: Timestamp }

struct Vault { vault_id: Uuid, entries: BTreeMap<Uuid, Entry> } // triées par id : ordre déterministe
```

- `pswd-core` ne lit jamais l'horloge : l'heure courante est fournie par l'appelant.
- Supprimer une entrée : `deleted = true`, `updated_at` mis à jour, champs de contenu et historique vidés. Les tombstones sont conservés.
- Toute modification pousse l'état précédent dans `history`.

## 5. Format du fichier `vault.enc`

Binaire, little-endian, versionné.

```
magic            "PSWD"            4 o
format_version   u16               (1)
keys_updated_at  i64 (ms UTC)      date de la dernière modification des enveloppes
envelope_count   u8
envelopes[]      voir ci-dessous
payload_nonce    24 o
payload          XChaCha20-Poly1305(vault_key, JSON du Vault), AAD = tout l'en-tête qui précède
```

**Clé du coffre** : 32 octets aléatoires, générée à `init`, chiffre le payload. Elle n'est jamais stockée en clair ; elle est « enveloppée » par une ou plusieurs clés d'enveloppe :

| `kind` | Enveloppe | Clé d'enveloppe |
|---|---|---|
| 1 | Mot de passe | Argon2id(mot de passe, sel) ; contient `m_cost`, `t_cost`, `p`, `salt` (16 o) |
| 2 | Récupération | HKDF-SHA256(clé de récupération, info = `"pswd-recovery-v1"`) |
| 3 | *Réservé* : mot de passe + fichier-clé | — |

Chaque enveloppe : `kind u8 | paramètres propres | nonce 24 o | XChaCha20-Poly1305(clé d'enveloppe, vault_key)`.

- Paramètres Argon2id par défaut : 64 MiB, 3 itérations, parallélisme 1 (compatible mobile). Ils sont lus depuis le fichier, donc modifiables sans migration. Bornes acceptées (contre un fichier malveillant) : mémoire 8 MiB – 1 GiB, 1 à 10 itérations, parallélisme 1 à 8.
- Aléa (clés, sels, nonces) : `getrandom` (générateur du système).
- Clé de récupération : 32 octets aléatoires, affichés **une seule fois** à `init`, encodés en Base32 Crockford par groupes de 4 caractères.
- Tout fichier mal formé, version inconnue ou échec d'authentification → erreur explicite, jamais de panique.
- Le JSON déchiffré est stocké dans un buffer effacé (`zeroize`) après usage.

## 6. Fusion

Fonction pure `merge(local: Vault, remote: Vault) -> Result<Vault, MergeError>` dans `pswd-core`. Refuse deux coffres d'identifiants différents.

1. Union des entrées par `id`.
2. Entrée présente d'un seul côté → conservée.
3. Présente des deux côtés et identique → conservée.
4. Sinon, la gagnante est celle dont `updated_at` est le plus grand ; à date égale, une entrée vivante l'emporte sur une tombstone, puis départage déterministe par comparaison champ par champ du contenu (pas de hash : il devrait être stable entre versions du compilateur). `created_at` = le plus ancien, `deleted_at` = le plus récent.
5. Historique = union des deux historiques + version perdante, puis : retrait des versions datées au plus `deleted_at` (une tombstone n'est donc jamais historisée et n'a pas d'historique) et de la version courante, tri (date puis contenu, plus récentes d'abord), dédoublonnage, troncature à 10.
6. Une suppression suit la même règle : une modification postérieure à une suppression fait revivre l'entrée, sans ses versions antérieures à la suppression. `deleted_at` est nécessaire à l'associativité : sans lui, l'historique dépendrait de l'ordre des fusions.

**Propriétés obligatoires** (testées avec `proptest`) : commutative, associative, idempotente ; aucune entrée non supprimée ne disparaît.

**En-tête** : les enveloppes retenues sont celles du fichier dont `keys_updated_at` est le plus récent (propage un changement de mot de passe). La clé du coffre est identique des deux côtés, puisque `passwd` et `recover` ne la changent pas.

**Limite acceptée** : `updated_at` dépend de l'horloge de l'appareil ; une horloge très décalée peut faire gagner la mauvaise version (la perdante reste dans l'historique).

## 7. Synchro et cycle de vie

**Emplacements** (dossiers standard de l'OS, crate `directories`)
- Config : `<config_dir>/pswd/config.toml`, contenant les coffres nommés (`[vaults.<nom>]` : URL du dépôt, chemin local) et `default_vault`, `auto_lock_minutes = 10`, `clipboard_clear_seconds = 20`.
- Données : `<data_dir>/pswd/vaults/<nom>/` = clone Git contenant `vault.enc`.
- Jeton d'accès : trousseau de l'OS, service `pswd`, compte `<nom>`. Jamais en clair sur le disque.

**Hébergement** : 1 dépôt Git privé = 1 coffre. HTTPS + jeton GitHub *fine-grained* limité à ce seul dépôt (Contents : lecture/écriture). Plus tard, un serveur maison (Gitea…) avec un compte et un dépôt par personne.

**Déroulé**
1. **Ouverture** : `fetch` avec un délai max de 5 s. En cas de succès : déchiffrement, fusion, écriture locale. En cas d'échec : ouverture de la copie locale avec `⚠ Hors ligne — dernière synchro : <date>`.
2. **Chaque modification** : écriture locale atomique + commit, puis tentative de push. En cas d'échec, l'état passe à « modifications en attente ».
3. **Push rejeté** (le distant a bougé) : fetch → fusion → nouveau push, 3 tentatives max.
4. **Verrouillage / fermeture** : nouvelle tentative de push s'il y a des modifications en attente.
5. **`sync`** manuel à tout moment.

## 8. CLI

**Commandes ponctuelles**

| Commande | Effet |
|---|---|
| `pswd init <nom>` | Crée un coffre : mot de passe maître (saisi 2 fois), URL du dépôt, jeton, affichage unique de la clé de récupération, premier push. **Refuse** si un coffre existe déjà localement ou si le dépôt distant contient déjà `vault.enc` (et propose `join`). |
| `pswd join <nom> <url>` | Clone un coffre existant, demande le jeton, vérifie le mot de passe maître. |
| `pswd open [nom]` | Déverrouille et lance le shell interactif. |
| `pswd sync [nom]` | Synchronise. |
| `pswd passwd [nom]` | Change le mot de passe maître (ancien requis), refait l'enveloppe 1. |
| `pswd recover [nom]` | Clé de récupération + nouveau mot de passe, refait l'enveloppe 1. |
| `pswd import pass <fichier.csv> [--vault nom]` | Importe le format CSV ci-dessous, puis rappelle de supprimer le fichier. |
| `pswd export <fichier.csv> [--vault nom]` | Export en clair : double confirmation + mot de passe maître redemandé. |

Aucune commande n'écrase un coffre existant.

**Shell interactif** : `list [--tag t]`, `search <texte> [--tag t]`, `show <entrée> [--show]`, `copy <entrée>`, `add [--generate]`, `edit <entrée>`, `rm <entrée>`, `history <entrée>`, `tags`, `gen [--length N] [--no-symbols]`, `sync`, `lock`, `quit`.

- **Verrouillage automatique** après 10 min d'inactivité : la clé est effacée de la mémoire à l'échéance, même si aucune commande n'est tapée.
- **Recherche** : sans tenir compte de la casse, sur le titre, l'identifiant, les URLs et les tags ; les tombstones sont ignorés.
- **Générateur** : `OsRng`, 24 caractères par défaut, majuscules, minuscules, chiffres et symboles ; au moins un caractère de chaque classe active.
- **Affichage** : mot de passe masqué par défaut ; le mot de passe maître n'est jamais affiché ni copié.

**Format CSV (import et export, aller-retour sans perte)**

```csv
title,kind,username,password,urls,notes,tags
GitHub,login,arthur,xxxx,https://github.com,,dev;perso
Code Wi-Fi,note,,,,"Box: ABCD-1234",maison
```
`kind` ∈ {`login`, `note`} ; `urls` et `tags` séparés par `;`. RFC 4180 (guillemets, retours à la ligne dans les champs). Une ligne invalide fait échouer l'import avec son numéro de ligne ; rien n'est importé partiellement.

## 9. Sécurité

- Secrets (`password`, clés, mot de passe maître, JSON déchiffré) dans des types effacés à la libération (`zeroize` / `secrecy`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- **Presse-papiers** : seulement les mots de passe d'entrées ; effacement après 20 s, uniquement s'il contient encore la valeur copiée. Le contenu doit être exclu de l'historique dès la copie :
  - Windows : formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`, `CanUploadToCloudClipboard = 0` ;
  - Linux : type MIME `x-kde-passwordManagerHint = secret` (partiel : avertissement affiché) ;
  - Android 13+ (futur) : `EXTRA_IS_SENSITIVE` ; iOS (futur) : `localOnly` + `expirationDate`.
- Mot de passe maître **et** clé de récupération perdus = coffre perdu. Aucune récupération possible côté serveur.
- **Historique Git** : après `passwd` / `recover`, l'ancien mot de passe ouvre toujours les anciennes versions du fichier présentes dans l'historique. Si le mot de passe a fuité, une rotation de clé est nécessaire (hors v1).

## 10. Points d'extension à préparer

Chaque point est marqué dans le code par un commentaire `// EXTENSION(<nom>): …` à l'endroit exact où il se branchera.

| Nom | Préparation en v1 |
|---|---|
| `keyfile` | `kind = 3` réservé dans les enveloppes ; dérivation de la clé d'enveloppe isolée pour accepter un second secret ; port de déverrouillage générique. |
| `key-rotation` | Les enveloppes sont indépendantes du payload ; `pswd passwd --rotate-key` génèrera une nouvelle clé de coffre et rechiffrera. |
| `mobile` | `pswd-core` sans E/S ; ports `SecureClipboard` / `SecretStore` à réimplémenter. |
| `remote-transport` | `RemoteStore` abstrait ; adaptateur « dossier synchronisé » possible. |
| `shared-vault` | Plusieurs enveloppes possibles par coffre (une par membre, à clé publique). Exclu pour l'instant, mais non bloqué par le format. |
