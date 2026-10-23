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
- `LocalStore` : `load() -> Option<bytes>` (`None` : pas de coffre local), `save_atomic(bytes)` (fichier temporaire + renommage).
- `RemoteStore` : `fetch() -> Option<(bytes, version)>` (`None` : distant vide ; une erreur vaut « hors ligne », l'adaptateur borne lui-même l'attente), `push(bytes, expected_version)` (`Conflict` si le distant a bougé ; `expected = None` exige un distant vide).
- `SecretStore` : lire / écrire le jeton d'accès distant d'un coffre.
- `SecureClipboard` : `copy_secret(value, ttl)`.
- Les erreurs des adaptateurs sont des `PortError` (un message, jamais de secret).

**Session** (`pswd-core`) : coffre ouvert, déverrouillé par mot de passe ou clé de récupération (`Credentials`, extensible au fichier-clé). Elle applique le déroulé du §7 avec n'importe quels adaptateurs, ce qui la rend testable avec des magasins en mémoire et réutilisable sur mobile.

Avec Git, `LocalStore` et `RemoteStore` visent le même clone local ; ils restent distincts pour permettre un transport sans Git.

La crypto n'est **pas** un port : déterministe, sans E/S, identique partout.

**Dépendances prévues** : `argon2`, `chacha20poly1305`, `hkdf` + `sha2`, `getrandom`, `rand` (générateur de mots de passe), `zeroize` / `secrecy`, `serde` + `serde_json`, `csv`, `uuid`, `keyring-core` (+ un magasin par OS : Windows, macOS, Secret Service sous Linux), `etcetera` (dossiers standard, licence MIT/Apache, contrairement à `directories` qui tire du MPL-2.0), `toml`, `clap`, `arboard` (presse-papiers). Librairie Git embarquée : `git2` (libgit2), retenue pour son push HTTPS éprouvé ; `gix` ne sait pas encore pousser (à réévaluer, le port `RemoteStore` permet d'en changer). Sur Android, `git2` exige OpenSSL embarqué (`vendored-openssl`). Jamais le binaire `git` du système.

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
payload          XChaCha20-Poly1305(vault_key, JSON du Vault), AAD = en-tête de `magic` à la fin des enveloppes
```

**Clé du coffre** : 32 octets aléatoires, générée à `init`, chiffre le payload. Elle n'est jamais stockée en clair ; elle est « enveloppée » par une ou plusieurs clés d'enveloppe :

| `kind` | Enveloppe | Clé d'enveloppe |
|---|---|---|
| 1 | Mot de passe | Argon2id(mot de passe, sel) ; contient `m_cost`, `t_cost`, `p`, `salt` (16 o) |
| 2 | Récupération | HKDF-SHA256(clé de récupération, info = `"pswd-recovery-v1"`) |
| 3 | *Réservé* : mot de passe + fichier-clé | — |

Chaque enveloppe : `kind u8 | paramètres propres | nonce 24 o | XChaCha20-Poly1305(clé d'enveloppe, vault_key)` (48 o). `kind` et les paramètres forment l'AAD de l'enveloppe. Paramètres du kind 1 : `m_cost u32 | t_cost u32 | p u32 | salt 16 o` ; le kind 2 n'en a pas.

En version 1, l'en-tête contient exactement une enveloppe 1 et une enveloppe 2, dans n'importe quel ordre. Un `kind` inconnu est refusé (sa taille est inconnue) ; une enveloppe absente ou en double rend le fichier mal formé. Limiter à une enveloppe mot de passe évite qu'un fichier piégé impose des centaines de calculs Argon2. Le fichier est entièrement découpé et validé avant toute dérivation de clé.

- Paramètres Argon2id par défaut : 64 MiB, 3 itérations, parallélisme 1 (compatible mobile). Ils sont lus depuis le fichier, donc modifiables sans migration. Bornes acceptées (contre un fichier malveillant) : mémoire 8 MiB – 1 GiB, 1 à 10 itérations, parallélisme 1 à 8.
- Aléa (clés, sels, nonces) : `getrandom` (générateur du système).
- Clé de récupération : 32 octets aléatoires, affichés **une seule fois** à `init`, encodés en Base32 Crockford par groupes de 4 caractères : 52 caractères en 13 groupes séparés par `-`. À la saisie, tirets, espaces et casse sont ignorés, `O` est lu `0`, `I` et `L` sont lus `1` ; les 4 bits de complément doivent être nuls (une seule écriture par clé).
- Tout fichier mal formé, version inconnue ou échec d'authentification → erreur explicite, jamais de panique.
- Le JSON déchiffré est stocké dans un buffer effacé (`zeroize`) après usage. À l'écriture, sa taille est calculée d'abord pour que le buffer ne soit jamais réalloué (une réallocation laisserait une copie non effacée).
- Payload : `{"vault_id": …, "entries": [Entry, …]}`, champs en `snake_case`, `kind` en minuscules, dates en millisecondes. Champ inconnu ou entrée en double → fichier mal formé (un champ inconnu serait sinon perdu à la réécriture). Les erreurs de lecture JSON ne reprennent jamais le détail de serde, qui peut citer le contenu.

## 6. Fusion

Fonction pure `merge(local: Vault, remote: Vault) -> Result<Vault, MergeError>` dans `pswd-core`. Refuse deux coffres d'identifiants différents.

1. Union des entrées par `id`.
2. Entrée présente d'un seul côté → conservée.
3. Présente des deux côtés et identique → conservée.
4. Sinon, la gagnante est celle dont `updated_at` est le plus grand ; à date égale, une entrée vivante l'emporte sur une tombstone, puis départage déterministe par comparaison champ par champ du contenu (pas de hash : il devrait être stable entre versions du compilateur). `created_at` = le plus ancien, `deleted_at` = le plus récent.
5. Historique = union des deux historiques + version perdante, puis : retrait des versions datées au plus `deleted_at` (une tombstone n'est donc jamais historisée et n'a pas d'historique) et de la version courante, tri (date puis contenu, plus récentes d'abord), dédoublonnage, troncature à 10.
6. Une suppression suit la même règle : une modification postérieure à une suppression fait revivre l'entrée, sans ses versions antérieures à la suppression. `deleted_at` est nécessaire à l'associativité : sans lui, l'historique dépendrait de l'ordre des fusions.

**Propriétés obligatoires** (testées avec `proptest`) : commutative, associative, idempotente ; aucune entrée non supprimée ne disparaît.

**En-tête** : les enveloppes retenues sont celles du fichier dont `keys_updated_at` est le plus récent (propage un changement de mot de passe) ; à date égale, les octets de l'en-tête départagent, pour que tous les appareils convergent. Celles du fichier distant ne sont adoptées qu'après déchiffrement de son payload (l'en-tête est authentifié comme AAD). La clé du coffre est identique des deux côtés, puisque `passwd` et `recover` ne la changent pas ; un fichier chiffré par une autre clé est refusé comme altéré, un coffre d'un autre identifiant comme « autre coffre ».

**Changement de mot de passe** (`passwd`, `recover`) : nouvelle enveloppe 1 avec un nouveau sel ; l'enveloppe 2 est conservée. `keys_updated_at` = max(maintenant, valeur actuelle + 1 ms), pour que le changement l'emporte même si l'horloge a reculé.

**Limite acceptée** : `updated_at` dépend de l'horloge de l'appareil ; une horloge très décalée peut faire gagner la mauvaise version (la perdante reste dans l'historique).

## 7. Synchro et cycle de vie

**Emplacements** (dossiers standard de l'OS, crate `etcetera` : `AppData\Roaming` sous Windows, `~/.config` et `~/.local/share` ailleurs)
- Config : `<config_dir>/pswd/config.toml`, contenant les coffres nommés (`[vaults.<nom>]` : URL du dépôt, chemin local) et `default_vault`, `auto_lock_minutes = 10`, `clipboard_clear_seconds = 20`.
- Données : un dossier par coffre, `<data_dir>/pswd/vaults/<nom>/`, contenant `vault.enc` (copie locale, `FileStore`) et `repo.git/` (dépôt Git nu, cache du distant, `GitRemote`). Plusieurs coffres coexistent ; aucun ne remplace un autre.
- Dépôt distant : branche `main`, fichier `vault.enc` à la racine ; les autres fichiers (README créé par GitHub) sont conservés. Chaque publication est un commit dont le parent est la version récupérée : le serveur refuse tout push qui n'est pas une avance rapide, ce qui garantit qu'aucune version n'est écrasée. Jeton présenté une seule fois par opération (utilisateur `x-access-token`).
- Délai max de 5 s : à configurer à l'échelle du processus via les options de libgit2 (`set_server_connect_timeout_in_milliseconds`, fonction `unsafe`) lors du câblage de la CLI.
- Nom d'un coffre : lettres ASCII, chiffres, `-` et `_`, 64 caractères au plus, hors noms réservés de Windows (`con`, `nul`…) ; il sert de nom de dossier et de compte dans le trousseau. Le premier coffre ajouté devient le coffre par défaut ; un nom n'est jamais réattribué.
- Jeton d'accès : trousseau de l'OS, service `pswd`, compte `<nom>`. Jamais en clair sur le disque.

**Hébergement** : 1 dépôt Git privé = 1 coffre. HTTPS + jeton GitHub *fine-grained* limité à ce seul dépôt (Contents : lecture/écriture). Plus tard, un serveur maison (Gitea…) avec un compte et un dépôt par personne.

**Déroulé**
1. **Ouverture** : `fetch` avec un délai max de 5 s. En cas de succès : déchiffrement, fusion, écriture locale. En cas d'échec : ouverture de la copie locale avec `⚠ Hors ligne — dernière synchro : <date>`.
2. **Chaque modification** : écriture locale atomique, puis tentative de publication (commit + push dans le dépôt nu). En cas d'échec, l'état passe à « modifications en attente ».
3. **Push rejeté** (le distant a bougé) : fetch → fusion → nouveau push, 3 tentatives max.
4. **Verrouillage / fermeture** : nouvelle tentative de push s'il y a des modifications en attente.
5. **`sync`** manuel à tout moment.

Précisions :
- Copie locale et distant ne sont réécrits que s'ils sont en retard sur la fusion : ouvrir un coffre à jour ne crée ni écriture ni commit, et une modification sans effet n'écrit rien.
- Si l'écriture locale d'une modification échoue, le coffre en mémoire revient à son état précédent.
- Un fichier distant inutilisable (altéré, autre coffre, version inconnue) n'est **jamais écrasé** : état « refusé », le coffre local reste utilisable.
- États de synchro : à jour, hors ligne (avec le message de l'adaptateur), conflit persistant après 3 tentatives, distant refusé.

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
| `pswd import <fichier.csv> [--vault nom]` | Importe le format CSV ci-dessous (tout le fichier est validé avant d'ajouter quoi que ce soit), puis rappelle de supprimer le fichier. |
| `pswd export <fichier.csv> [--vault nom]` | Export en clair : double confirmation + mot de passe maître redemandé. Un fichier existant n'est jamais écrasé. |

Aucune commande n'écrase un coffre existant.

`init` et `join` vérifient le nom du coffre avant toute question. Un essai raté ne laisse rien derrière lui : dossier du coffre supprimé (s'il n'existait pas avant), configuration et trousseau inchangés. Le jeton est facultatif (saisie vide : aucun jeton, par exemple pour un dépôt local).

**Shell interactif** : `list [--tag t]`, `search <texte> [--tag t]`, `show <entrée> [--show]`, `copy <entrée>`, `add [--generate]`, `edit <entrée>`, `rm <entrée>`, `history <entrée>`, `tags`, `gen [--length N] [--no-symbols]`, `sync`, `lock`, `quit`.

- **Désigner une entrée** (`show`, `copy`…) : par son titre, sans tenir compte de la casse, ou par le début de son identifiant (affiché par `list`). Si plusieurs entrées correspondent, la commande refuse et les liste. Une erreur dans le shell est affichée et le shell continue ; fin de l'entrée standard = `quit`.
- **`add` / `edit`** : questions champ par champ (une note n'a ni identifiant, ni mot de passe, ni URL) ; URLs et tags séparés par des espaces. Dans `edit`, la valeur actuelle est rappelée : réponse vide = inchangé, `-` = effacer. `--generate` crée le mot de passe sans l'afficher (`copy` pour le copier). `rm` demande confirmation (non par défaut). `history` date chaque version (UTC) et signale un mot de passe différent. En quittant, les modifications non publiées sont renvoyées.
- **Presse-papiers** ouvert à la première copie : une machine sans presse-papiers peut utiliser toutes les autres commandes.
- **Verrouillage automatique** après 10 min d'inactivité : la clé est effacée de la mémoire à l'échéance, même si aucune commande n'est tapée.
- **Recherche** : sans tenir compte de la casse, sur le titre, l'identifiant, les URLs et les tags ; les tombstones sont ignorés.
- **Générateur** : 24 caractères par défaut (8 à 128), majuscules, minuscules, chiffres et symboles `!#$%&()*+,-./:;<=>?@[]^_{|}~` (ni espace, ni guillemets, ni antislash, ni accent grave) ; au moins un caractère de chaque classe active, obtenu par rejet des candidats incomplets pour que tous les mots de passe valides restent équiprobables. `pswd-core` reçoit le générateur aléatoire en paramètre, contraint par le trait `CryptoRng` ; la CLI fournit un `StdRng` initialisé par `SysRng` (aléa du système, `rand` 0.10) : si l'aléa manque, une erreur est affichée, jamais de panique.
- **Affichage** : mot de passe masqué par défaut ; le mot de passe maître n'est jamais affiché ni copié.

**Format CSV (import et export, aller-retour sans perte)**

```csv
title,kind,username,password,urls,notes,tags
GitHub,login,arthur,xxxx,https://github.com,,dev;perso
Code Wi-Fi,note,,,,"Box: ABCD-1234",maison
```
`kind` ∈ {`login`, `note`} ; `urls` et `tags` séparés par `;`. RFC 4180 (guillemets, retours à la ligne dans les champs). Une ligne invalide fait échouer l'import avec son numéro de ligne ; rien n'est importé partiellement.

- Listes (`urls`, `tags`) : `\;` et `\\` désignent un `;` et un `\` littéraux ; un `\` isolé est gardé tel quel ; les éléments vides sont ignorés.
- Une cellule vide est un champ absent (`username`, `password`, `notes`). `kind` est lu sans tenir compte de la casse. Le BOM UTF-8 (Excel) est ignoré ; fins de ligne `\r\n`, `\n` ou `\r`.
- L'en-tête doit être exactement celui ci-dessus. Les numéros de ligne sont ceux du fichier, y compris dans les champs sur plusieurs lignes ; ils sont calculés par `pswd-core` (ceux de la crate `csv` sont faux avec `\r\n` et les lignes vides). Les messages d'erreur ne citent jamais le contenu.
- Export : entrées non supprimées, sans historique, triées par contenu, fins de ligne `\r\n`. Un champ commençant par `=`, `+`, `-` ou `@` n'est pas neutralisé (l'aller-retour serait perdu) : ouvrir l'export dans un tableur peut l'interpréter comme une formule.
- Limite acceptée : les buffers internes de la crate `csv` ne sont pas effacés ; le fichier CSV est de toute façon en clair.

## 9. Sécurité

- Secrets (`password`, clés, mot de passe maître, JSON déchiffré) dans des types effacés à la libération (`zeroize` / `secrecy`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- **Presse-papiers** : seulement les mots de passe d'entrées ; effacement après 20 s, ou dès la fermeture du programme si elle survient avant, uniquement s'il contient encore la valeur copiée ; seul le minuteur de la dernière copie efface. Le contenu doit être exclu de l'historique dès la copie :
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
