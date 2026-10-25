# SPEC — pswd

Référence de conception. Toute décision qui change ce document doit y être reportée.

## 1. Objectif

Gestionnaire de mots de passe personnel, local-first, pour remplacer un service tiers (NordPass).
L'utilisateur reste maître de ses données : elles vivent sur ses appareils, sur ses disques de sauvegarde et, s'il le souhaite, sur son propre serveur. Aucun service tiers n'intervient.
Le coffre est chiffré de bout en bout : le serveur ne voit jamais de données en clair ni de clé.

## 2. Feuille de route

**Étape 1 — local complet** (terminée)
- CLI `pswd` sous Windows et Linux.
- Plusieurs coffres indépendants et entièrement locaux (un mot de passe maître et une clé par coffre).
- Entrées `Login` et `Note`, tags, historique des versions.
- Générateur de mots de passe, copie sécurisée dans le presse-papiers.
- Changement de mot de passe maître, clé de récupération.
- Import CSV (format §8). Pas d'export en clair : il exposerait tous les mots de passe dans un fichier, sans besoin réel (les sauvegardes, elles, restent chiffrées).
- Sauvegarde et restauration sur disque externe, suppression d'un coffre (§7).

**Étape 2 — interfaces graphiques** (en cours, §12) : application **PSWD Master** pour le bureau (Windows, Linux, macOS) et le mobile (Android, iOS), un seul projet Tauri 2. Chaque appareil est autonome et entièrement local, avec les fonctionnalités de la CLI. Sous-étapes, chacune validée et commitée :
1. `pswd-app` : cas d'usage extraits de la CLI, comportement inchangé.
2. Squelette Tauri (`pswd-gui`) : CSP et capacités, premier build Android et iOS ; liste des coffres, création, déverrouillage, verrouillage.
3. Consultation : liste, recherche, tags, détail, copier, afficher. Reportés : ouvrir une URL dans le navigateur du système (plugin et capacité dédiés), copier l'identifiant.
4. Modification : ajout, édition, suppression, historique, générateur.
5. Gestion des coffres : mot de passe maître, récupération, import CSV, sauvegarde et restauration, suppression, réglages.
6. Mobile : presse-papiers sensible, verrouillage en arrière-plan, écran masqué, fichiers `.pswd`, ergonomie tactile.
7. Installateurs, documentation, mutants.

**Étape 3 — serveur maison** (§11) : `pswd-server`, comptes, coffres privés et communs, synchro entre appareils.

La v1 synchronisait chaque coffre avec un dépôt Git privé (GitHub). Cette synchro a été retirée au profit du serveur maison ; son code reste dans l'historique du dépôt (jusqu'au commit `233f829`).

**Hors feuille de route (le terrain doit être préparé, cf. §10)**
- Fichier-clé comme second facteur.
- Rotation de la clé du coffre.
- Partage par membre, synchro par dossier réseau, plusieurs serveurs par appareil.

**Exclu**
- Codes TOTP, champs personnalisés, extension navigateur / autofill.

## 3. Architecture

Workspace Cargo, architecture hexagonale :

| Crate | Contenu | Contraintes |
|---|---|---|
| `pswd-core` | Modèles, fusion, formats de fichier (`vault.enc`, `.pswd`), noms de coffre, crypto, générateur, CSV, ports (traits) | Aucune E/S, aucun appel OS, `#![forbid(unsafe_code)]`, compilable pour mobile |
| `pswd-infra` | Adaptateurs : `LocalStore` fichier, `SecureClipboard` du bureau, config, sauvegardes | `arboard` réservé au bureau : la crate compile pour le mobile |
| `pswd-app` | Cas d'usage partagés : créer, ouvrir, supprimer, sauvegarder, restaurer (à partir d'octets), importer, lister les coffres. Ne pose aucune question à l'utilisateur. | `#![forbid(unsafe_code)]` |
| `pswd-cli` | Binaire `pswd` : questions et affichage dans le terminal, shell interactif, verrouillage automatique | `#![forbid(unsafe_code)]` |
| `pswd-gui` | Application Tauri 2 **PSWD Master** : backend Rust (commandes, session, verrouillage) et interface Svelte 5 + TypeScript (§12) | `#![forbid(unsafe_code)]` |

Futur : `pswd-server` et l'adaptateur `RemoteStore` HTTP (étape 3).

**Ports (définis dans `pswd-core`)**
- `LocalStore` : `load() -> Option<bytes>` (`None` : pas de coffre local), `save_atomic(bytes)` (fichier temporaire + renommage).
- `RemoteStore` (étape 3) : `fetch() -> Option<(bytes, version)>` (`None` : distant vide ; une erreur vaut « hors ligne », l'adaptateur borne lui-même l'attente), `push(bytes, expected_version)` (`Conflict` si le distant a bougé ; `expected = None` exige un distant vide).
- `SecureClipboard` : `copy_secret(value, ttl)`.
- Les erreurs des adaptateurs sont des `PortError` (un message, jamais de secret).

**Session** (`pswd-core`) : coffre ouvert, déverrouillé par mot de passe ou clé de récupération (`Credentials`, extensible au fichier-clé). Toute modification est enregistrée localement et marque le coffre comme ayant des modifications non publiées. Avant chaque écriture (modification, changement de mot de passe, synchro), la copie locale est relue : si un autre programme l'a réécrite depuis la dernière lecture ou écriture de la session, elle est fusionnée (§6) ; sinon elle ne l'est pas, car refusionner sa propre écriture, plus ancienne, pourrait annuler à date égale la dernière modification. Un mot de passe changé ailleurs est remplacé par le nouveau. Une copie locale disparue ou inutilisable n'est jamais remplacée : l'écriture est refusée. La synchro (§7) est une opération distincte qui reçoit le distant en paramètre : une session n'a besoin d'aucun distant. Les magasins en mémoire la rendent testable, et elle sera réutilisée par les interfaces graphiques.

La crypto n'est **pas** un port : déterministe, sans E/S, identique partout.

**Dépendances prévues** : `argon2`, `chacha20poly1305`, `hkdf` + `sha2`, `getrandom`, `rand` (générateur de mots de passe), `zeroize` / `secrecy`, `serde` + `serde_json`, `csv`, `uuid`, `etcetera` (dossiers standard, licence MIT/Apache, contrairement à `directories` qui tire du MPL-2.0), `toml`, `clap`, `arboard` (presse-papiers du bureau), `tauri` 2. Côté interface (npm) : `svelte` 5, `vite`, `typescript`, `vitest`, Testing Library, `svelte-check`, ESLint ; aucune bibliothèque de composants graphiques.

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

**Restaurer n'est pas fusionner** : une ancienne copie (sauvegarde, version conservée par le serveur) n'est jamais fusionnée dans le coffre actuel. Ses entrées supprimées depuis seraient aussitôt ré-effacées par les tombstones, plus récentes. Une restauration crée toujours un nouveau coffre.

## 7. Stockage, sauvegarde et synchro

**Emplacements** (dossiers standard de l'OS, crate `etcetera` : `AppData\Roaming` sous Windows, `~/.config` et `~/.local/share` ailleurs)
- Config : `<config_dir>/pswd/config.toml`, contenant les coffres nommés (`[vaults.<nom>]` : chemin local), `auto_lock_minutes = 10`, `clipboard_clear_seconds = 20`.
- Données : un dossier par coffre, `<data_dir>/pswd/vaults/<nom>/`, contenant `vault.enc` (`FileStore`). Plusieurs coffres coexistent ; aucun ne remplace un autre.
- Nom d'un coffre : lettres ASCII, chiffres, `-` et `_`, 64 caractères au plus, hors noms réservés de Windows (`con`, `nul`…) ; il sert de nom de dossier. Un nom pris est refusé tant que le coffre existe. Il n'y a pas de coffre par défaut : chaque commande nomme son coffre.
- Les restes de la v1 (dossiers `repo.git/`, jetons `pswd` du trousseau) ne sont plus utilisés et peuvent être supprimés à la main ; les entrées `url` et `default_vault` de `config.toml` doivent être retirées.

**Sauvegarde** : un seul format sur tous les appareils, un fichier `.pswd` (sur mobile, une sauvegarde ne peut pas être un dossier). Il sert aussi à passer ses coffres d'un appareil à l'autre.
- Contenu : tous les coffres locaux (ou un seul, au choix, dans la CLI), copiés bruts, toujours chiffrés : aucun mot de passe demandé, rien de lisible sur le disque. Nom proposé : `pswd-sauvegarde_<AAAA-MM-JJ_HH-MM-SS>.pswd` (UTC). Les noms des coffres sont rangés dans le fichier : un sélecteur de fichiers mobile ne donne pas toujours le nom du fichier, et celui-ci a pu être renommé.
- CLI : `pswd backup <dossier>` écrit ce fichier dans le dossier, qui doit exister : sinon, une clé débranchée donnerait une sauvegarde sur le disque de la machine (sous Linux, le point de montage est un dossier ordinaire). Un fichier existant n'est jamais remplacé ; une écriture ratée retire le fichier incomplet. Interface graphique : la fenêtre système d'enregistrement choisit l'emplacement (§12).
- Le fichier est sur le disque (`sync_all`) avant la fin de la commande. Supprimer les anciennes sauvegardes est laissé à l'utilisateur.
- Format (`pswd-core`, binaire, little-endian) :

```
magic          "PSWDSAVE"        8 o
format_version u8                (1)
created_at     i64 (ms UTC)      date de la sauvegarde
vault_count    u16               1 000 au plus
pour chaque coffre, noms triés et distincts :
  name_len     u8
  name         ASCII             nom de coffre valide (§7)
  file_len     u32               64 Mio au plus
  file         vault.enc         vérifié (structure, version) à la lecture
```

- Le fichier est lu en entier et chaque coffre vérifié avant d'en restaurer un ; rien après le dernier coffre. Une sauvegarde n'a qu'une écriture (noms triés) : le fuzzing vérifie que tout fichier accepté se réencode à l'identique.

**Restauration**
- L'utilisateur donne un fichier `.pswd` et, au choix, un coffre ; sinon les coffres de la sauvegarde sont listés et le nom est demandé.
- Le fichier est vérifié (structure, version) avant toute écriture ; un fichier altéré à l'intérieur du chiffrement ne se voit qu'à l'ouverture.
- Le coffre est restauré comme **nouveau coffre local** : sous son nom d'origine s'il est libre, sinon sous un nom distinct (`<nom>-sauvegarde-<AAAA-MM-JJ>`, refusé s'il est pris à son tour). Rien n'est jamais écrasé ni fusionné (§6).
- Il s'ouvre avec le mot de passe en vigueur à la date de la sauvegarde.

**Suppression d'un coffre**
- Le nom du coffre doit être retapé ; un rappel propose de faire une sauvegarde avant.
- Seuls `vault.enc` et un éventuel `vault.enc.tmp` sont supprimés, puis le dossier s'il est vide : le chemin vient de `config.toml`, modifiable à la main, donc un dossier n'est jamais effacé en entier.
- Fichiers d'abord, configuration ensuite : un échec de suppression ne change rien.
- Le nom redevient libre.
- À l'étape 3, un coffre privé propose « local seulement » ou « local et serveur » ; un coffre commun se quitte (§11).

**Synchro** (algorithme de `pswd-core`, utilisé à partir de l'étape 3)
1. `fetch`. Distant injoignable : état « hors ligne ».
2. Distant présent : déchiffrement, fusion, écriture locale si la copie locale est en retard. Si le distant est à jour, fin.
3. Publication avec la version récupérée. Rejetée (le distant a bougé) : retour en 1, 3 tentatives max.

Précisions :
- Copie locale et distant ne sont réécrits que s'ils sont en retard sur la fusion : synchroniser un coffre à jour n'écrit rien, et une modification sans effet n'écrit rien.
- Si l'écriture locale d'une modification échoue, le coffre en mémoire revient à son état précédent.
- Un fichier distant inutilisable (altéré, autre coffre, version inconnue) n'est **jamais écrasé** : état « refusé », le coffre local reste utilisable.
- États de synchro : à jour, hors ligne (avec le message de l'adaptateur), conflit persistant après 3 tentatives, distant refusé.
- Les moments de synchro (ouverture, modification, verrouillage) sont fixés avec le serveur (§11).

## 8. CLI

**Commandes ponctuelles**

| Commande | Effet |
|---|---|
| `pswd init <nom>` | Crée un coffre local : mot de passe maître (saisi 2 fois), affichage unique de la clé de récupération. **Refuse** si le nom est déjà pris. |
| `pswd open <nom>` | Déverrouille et lance le shell interactif. |
| `pswd passwd <nom>` | Change le mot de passe maître (ancien requis), refait l'enveloppe 1. |
| `pswd recover <nom>` | Clé de récupération + nouveau mot de passe, refait l'enveloppe 1. |
| `pswd import <nom> <fichier.csv>` | Importe le format CSV ci-dessous (tout le fichier est validé avant d'ajouter quoi que ce soit), puis rappelle de supprimer le fichier. |
| `pswd backup <dossier> [--vault nom]` | Sauvegarde tous les coffres (ou un seul) dans un nouveau fichier `.pswd` du dossier (§7). |
| `pswd restore <fichier.pswd> [nom]` | Restaure un coffre d'une sauvegarde comme nouveau coffre local (§7). |
| `pswd delete <nom>` | Supprime un coffre de cette machine, après avoir retapé son nom (§7). |

Aucune commande n'écrase un coffre existant.

`init` vérifie le nom du coffre avant toute question. Un essai raté ne laisse rien derrière lui : dossier du coffre supprimé (s'il n'existait pas avant), configuration inchangée.

**Shell interactif** : `list [--tag t]`, `search <texte> [--tag t]`, `show <entrée> [--show]`, `copy <entrée>`, `add [--generate]`, `edit <entrée>`, `rm <entrée>`, `history <entrée>`, `tags`, `gen [--length N] [--no-symbols]`, `lock`, `quit`.

- **Désigner une entrée** (`show`, `copy`…) : par son titre, sans tenir compte de la casse, ou par le début de son identifiant (affiché par `list`). Si plusieurs entrées correspondent, la commande refuse et les liste. Une erreur dans le shell est affichée et le shell continue ; fin de l'entrée standard = `quit`.
- **`add` / `edit`** : questions champ par champ (une note n'a ni identifiant, ni mot de passe, ni URL) ; URLs et tags séparés par des espaces. Dans `edit`, la valeur actuelle est rappelée : réponse vide = inchangé, `-` = effacer. `--generate` crée le mot de passe sans l'afficher (`copy` pour le copier). `rm` demande confirmation (non par défaut). `history` date chaque version (UTC) et signale un mot de passe différent.
- **Presse-papiers** ouvert à la première copie : une machine sans presse-papiers peut utiliser toutes les autres commandes.
- **Verrouillage automatique** après 10 min d'inactivité : la clé est effacée de la mémoire à l'échéance, même si aucune commande n'est tapée. Un thread de surveillance détient la session ; chaque commande repousse l'échéance ; à l'échéance, il détruit la session. La saisie suivante affiche « Coffre verrouillé » et ferme le shell. `auto_lock_minutes = 0` désactive le verrouillage ; `lock` (alias de `quit`) verrouille tout de suite.
- **Copie sous Linux** : un avertissement rappelle que l'exclusion de l'historique dépend du gestionnaire de presse-papiers.
- **Recherche** : sans tenir compte de la casse, sur le titre, l'identifiant, les URLs et les tags ; les tombstones sont ignorés.
- **Générateur** : 24 caractères par défaut (8 à 128), majuscules, minuscules, chiffres et symboles `!#$%&()*+,-./:;<=>?@[]^_{|}~` (ni espace, ni guillemets, ni antislash, ni accent grave) ; au moins un caractère de chaque classe active, obtenu par rejet des candidats incomplets pour que tous les mots de passe valides restent équiprobables. `pswd-core` reçoit le générateur aléatoire en paramètre, contraint par le trait `CryptoRng` ; la CLI fournit un `StdRng` initialisé par `SysRng` (aléa du système, `rand` 0.10) : si l'aléa manque, une erreur est affichée, jamais de panique.
- **Affichage** : mot de passe masqué par défaut ; le mot de passe maître n'est jamais affiché ni copié.

**Format CSV d'import**

```csv
title,kind,username,password,urls,notes,tags
GitHub,login,arthur,xxxx,https://github.com,,dev;perso
Code Wi-Fi,note,,,,"Box: ABCD-1234",maison
```
`kind` ∈ {`login`, `note`} ; `urls` et `tags` séparés par `;`. RFC 4180 (guillemets, retours à la ligne dans les champs). Une ligne invalide fait échouer l'import avec son numéro de ligne ; rien n'est importé partiellement.

- Listes (`urls`, `tags`) : `\;` et `\\` désignent un `;` et un `\` littéraux ; un `\` isolé est gardé tel quel ; les éléments vides sont ignorés.
- Une cellule vide est un champ absent (`username`, `password`, `notes`). `kind` est lu sans tenir compte de la casse. Le BOM UTF-8 (Excel) est ignoré ; fins de ligne `\r\n`, `\n` ou `\r`.
- L'en-tête doit être exactement celui ci-dessus. Les numéros de ligne sont ceux du fichier, y compris dans les champs sur plusieurs lignes ; ils sont calculés par `pswd-core` (ceux de la crate `csv` sont faux avec `\r\n` et les lignes vides). Les messages d'erreur ne citent jamais le contenu.
- L'application n'écrit jamais de CSV. Les tests et le fuzzing vérifient l'aller-retour avec un écrivain réservé aux tests (`#[cfg(any(test, fuzzing))]`) : tout CSV accepté se relit à l'identique.
- Limite acceptée : les buffers internes de la crate `csv` ne sont pas effacés ; le fichier CSV est de toute façon en clair.

## 9. Sécurité

- Secrets (`password`, clés, mot de passe maître, JSON déchiffré) dans des types effacés à la libération (`zeroize` / `secrecy`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- **Presse-papiers** : seulement les mots de passe d'entrées ; effacement après 20 s, ou dès la fermeture du programme si elle survient avant, uniquement s'il contient encore la valeur copiée (sauf sur Android quand il est illisible, §12) ; seul le minuteur de la dernière copie efface. Le contenu doit être exclu de l'historique dès la copie :
  - Windows : formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`, `CanUploadToCloudClipboard = 0` ;
  - Linux : type MIME `x-kde-passwordManagerHint = secret` (partiel : avertissement affiché) ;
  - Android 13+ (futur) : `EXTRA_IS_SENSITIVE` ; iOS (futur) : `localOnly` + `expirationDate`.
- Mot de passe maître **et** clé de récupération perdus = coffre perdu. Aucune récupération possible côté serveur.
- **Anciennes copies** : après `passwd` / `recover`, l'ancien mot de passe ouvre toujours les copies antérieures du fichier (sauvegardes externes, versions conservées par le serveur). Si le mot de passe a fuité, une rotation de clé est nécessaire (hors feuille de route).
- **Interface graphique** : JavaScript ne peut pas effacer une chaîne de la mémoire ; le moins de secrets possible traversent vers la webview (§12).
  - Jamais côté JS : clé du coffre, coffre déchiffré, mot de passe copié (la copie est faite par Rust).
  - Côté JS seulement sur demande explicite : mot de passe affiché (« Afficher », masqué à nouveau après 30 s), mot de passe généré affiché.
  - **Limite acceptée** : les saisies (mot de passe maître, mot de passe d'une entrée) passent par un champ HTML, donc par JS ; le champ est vidé dès l'envoi.
  - CSP stricte (ressources de l'application seulement, ni script externe, ni `eval`, ni réseau) ; capacités Tauri limitées à nos commandes (aucun accès fichiers ou shell depuis JS) ; un lien d'entrée s'ouvre dans le navigateur du système, jamais dans la webview.
- **Mobile** : écran masqué dans les applications récentes et captures interdites (Android `FLAG_SECURE`, écran neutre sur iOS).

## 10. Points d'extension à préparer

Chaque point est marqué dans le code par un commentaire `// EXTENSION(<nom>): …` à l'endroit exact où il se branchera, dès que cet endroit existe.

| Nom | Préparation |
|---|---|
| `keyfile` | `kind = 3` réservé dans les enveloppes ; dérivation de la clé d'enveloppe isolée pour accepter un second secret ; port de déverrouillage générique. |
| `key-rotation` | Les enveloppes sont indépendantes du payload ; `pswd passwd --rotate-key` génèrera une nouvelle clé de coffre et rechiffrera. |
| `mobile` | `pswd-core` sans E/S ; port `SecureClipboard` réimplémenté par un plugin Tauri maison (Kotlin, Swift) à l'étape 2. |
| `biometrie` | Déverrouillage par empreinte ou Face ID : une clé rangée dans le stockage sécurisé du téléphone (Keystore, Keychain), à concevoir à part. |
| `anglais` | Textes de l'interface regroupés par langue (`src/lib/i18n/fr.ts`) ; `en.ts` devra avoir les mêmes clés (vérifié par TypeScript). Les erreurs Rust remontent à l'interface sous forme de codes, traduits côté interface. |
| `remote-transport` | `RemoteStore` abstrait : l'adaptateur HTTP de `pswd-server` s'y branche (étape 3). |
| `partage-par-membre` | Plusieurs enveloppes possibles par coffre (une par membre, à clé publique) : ajouter ou retirer un membre sans partager de mot de passe. Les coffres communs de l'étape 3 utilisent un mot de passe partagé. |
| `dossier-reseau` | Synchro par un partage réseau (NAS) sans `pswd-server`. Un dossier ne sait pas refuser une écriture concurrente : il faudrait un fichier par appareil, fusionnés à la lecture. |
| `multi-serveurs` | Un appareil connecté à plusieurs serveurs : chaque coffre est rattaché à un serveur. |

## 11. Serveur maison (étape 3)

**Principe** : `pswd-server` est un exécutable Rust unique, sans dépendance, lancé sur une machine de l'utilisateur (serveur Linux, NAS, Raspberry Pi). C'est une passerelle : il stocke des fichiers `vault.enc` opaques et ne détient aucune clé. Toute la gestion se fait depuis l'application.

**Transport** : HTTPS. Le serveur génère son propre certificat ; l'application l'**épingle** (n'accepte que lui). Écriture conditionnelle : une publication est refusée si le coffre a changé depuis la version lue, ce qui conserve le contrat `RemoteStore`. Le serveur identifie les coffres par son propre identifiant, jamais par leur nom.

**Accès** : l'application retient une adresse locale et une adresse publique, essayées dans cet ordre (délai max 5 s). L'accès depuis l'extérieur (redirection de port, VPN) est documenté ; il n'y a pas de relais.

**Administration**
- Au premier démarrage, le serveur affiche une fois son adresse et une **clé administrateur**. Le premier appareil qui la présente devient administrateur ; personne d'autre ne le peut.
- Secours : sur la machine, `pswd-server reset-admin` affiche une nouvelle clé administrateur.
- Le panneau d'administration de l'application permet d'ajouter un membre, de supprimer un compte, de révoquer un appareil, de promouvoir un membre administrateur ou de lui retirer ce rôle, de supprimer un coffre, de rattacher quelqu'un à un coffre sans membre et de restaurer une version. Le dernier administrateur ne peut pas perdre son rôle ni être supprimé.

**Comptes et appareils**
- Ajouter un membre produit l'adresse du serveur et une clé à usage unique, à recopier dans l'application (ordinateur ou mobile). Cette clé contient aussi une partie de l'empreinte du certificat : l'application vérifie le serveur dès la première connexion, sans comparaison manuelle.
- Chaque appareil reçoit son propre jeton, révocable individuellement. Le compte n'a pas de mot de passe serveur : rien ne se confond avec le mot de passe maître.
- Un appareil est connecté à un seul serveur. Il peut s'en déconnecter (son jeton est révoqué) puis se connecter à nouveau.

**Coffres**
- Un coffre est **privé** ou **commun**. Un coffre commun est protégé par un mot de passe maître partagé, transmis hors du serveur.
- Tout membre d'un coffre commun peut inviter un utilisateur du même serveur, qui accepte ou refuse, et peut quitter le coffre. Tous les membres sont égaux ; seul un administrateur retire quelqu'un. Après un départ, changer le mot de passe (`passwd`) est conseillé : l'ancien membre le connaît.
- Être membre donne le droit de télécharger et de publier le fichier ; le lire exige le mot de passe du coffre.
- Le serveur ne supprime jamais un coffre de lui-même, même sans membre. Depuis l'application, supprimer un coffre privé propose « local seulement » ou « local et serveur » ; un coffre commun se quitte. Supprimer un coffre sur le serveur ne touche pas aux copies locales des appareils.
- Le serveur conserve les **15 dernières versions** de chaque coffre. Une restauration crée un nouveau coffre avec les mêmes membres (§6).

**État d'un coffre sur un appareil**

| État | Signification | Comportement |
|---|---|---|
| à publier | Créé ou restauré, inconnu du serveur | Publié comme nouveau coffre privé dès que le serveur est joignable |
| publié | Présent sur le serveur, synchronisé | Synchro normale (§7) |
| détaché | L'appareil s'est déconnecté du serveur | Utilisé en local ; synchro reprise à la reconnexion au même serveur (reconnu par son certificat) |
| retiré | Le serveur l'a supprimé ou en a retiré l'utilisateur | Gardé en local, jamais republié automatiquement : seulement sur action volontaire, comme nouveau coffre |

- À la déconnexion, l'application demande s'il faut garder une copie locale des coffres (oui par défaut). Sans copie, les données restent sur le serveur.
- En se connectant à un **autre** serveur, les coffres détachés ne sont pas publiés automatiquement ; les coffres « à publier » le sont.
- Le serveur distingue « coffre inconnu » de « coffre retiré » : `RemoteStore` reçoit pour cela une erreur dédiée.

## 12. Interfaces graphiques (étape 2)

**Application** : PSWD Master, identifiant `fr.arthurfontaine.pswdmaster` (modifiable jusqu'à une première publication sur un store). Un seul projet Tauri 2 pour Windows, Linux, macOS, Android et iOS ; une seule branche `dev-interface`. Les différences entre plateformes se règlent dans le code (`#[cfg(target_os = …)]`), jamais par des branches.

**Organisation** : `crates/pswd-gui/` contient l'interface (`package.json`, `src/`, Vite) et `crates/pswd-gui/src-tauri/` la crate Rust `pswd-gui` (membre du workspace), avec `tauri.conf.json`, `capabilities/` et le projet Android généré (`gen/android`). Les commandes appelables sont déclarées dans `build.rs` et autorisées une à une dans `capabilities/default.json` ; un test vérifie qu'une commande existante mais non autorisée est refusée.

**Fonctionnement** : la webview du système affiche l'interface (HTML/CSS/JS intégrés à l'exécutable, aucun serveur web, aucun réseau). L'interface appelle les commandes Tauri du backend Rust, qui s'appuie sur `pswd-app`. La session (clé du coffre, coffre déchiffré) reste en Rust. Règles de sécurité : §9.

**Interface** : Svelte 5 + TypeScript, construite par Vite ; CSS maison avec des variables (couleurs, espacements), sans bibliothèque de composants.
- Thème clair ou sombre selon le système, réglable (auto, clair, sombre). C'est le seul réglage de l'écran « Réglages » : les délais (verrouillage, presse-papiers, arrière-plan) gardent leurs valeurs par défaut, modifiables à la main dans `config.toml` sur le bureau.
- Mise en page de départ, appelée à évoluer : deux colonnes sur le bureau (liste à gauche, détail à droite), une colonne sur mobile (liste, puis détail en plein écran).
- Bureau entièrement utilisable au clavier (Tab, Entrée, Échap, `Ctrl+F`) ; contrastes suffisants.
- Tactile : cibles d'au moins 48 px sur un écran tactile (`pointer: coarse`), survol réservé aux pointeurs qui survolent, marges des zones sûres (affichage bord à bord d'Android 15+, encoche) ; les champs saisis tels quels (nom de coffre, identifiant, clé de récupération, adresses, tags, recherche) désactivent majuscule et correction automatiques (`src/lib/keyboard.ts`).
- Bouton retour d'Android (`src/lib/back.ts`) : chaque écran ouvert s'inscrit, le dernier répond en premier. Il referme le formulaire ou le panneau, puis l'entrée ; dans la liste des entrées, il verrouille le coffre ; ailleurs, il revient à la liste des coffres, d'où il ferme l'application. La clé de récupération ne se quitte que par « Continuer ». Permissions dans `capabilities/mobile.json`, limitée à Android (écoute du bouton, fermeture) ; un test vérifie qu'elle ne s'applique pas au bureau.
- Français d'abord ; tous les textes passent par `src/lib/i18n/<langue>.ts` (cf. `EXTENSION(anglais)`).

**Fichiers et dossiers**
- Bureau : les mêmes dossiers que la CLI (`Locations::system`) ; les deux programmes voient les mêmes coffres et peuvent modifier le même à la fois : chaque écriture fusionne d'abord la copie sur le disque (§3, Session). Un programme ne voit les modifications de l'autre qu'à sa prochaine écriture ou réouverture. **Limite acceptée** : sans verrou de fichier, deux écritures dans la même fraction de seconde peuvent encore se recouvrir.
- Mobile : le dossier privé de l'application fourni par Tauri (`app_data_dir`), passé à `Locations::new` ; `FileStore` fonctionne tel quel.
- Sauvegarde : un fichier `.pswd` (§7), sur le bureau comme sur mobile. « Sauvegarder » ouvre la fenêtre système d'enregistrement ; « Restaurer une sauvegarde » ouvre le sélecteur de fichiers, liste les coffres du fichier, puis applique les règles du §7. `pswd-app` restaure à partir des octets du fichier, pas d'un chemin. Les fichiers choisis sont lus et écrits par Rust (module `files`), jamais par l'interface ; une écriture se termine par `sync_all`.
- Les fenêtres système du mobile renvoient une adresse (`content://` sur Android), pas un chemin : Rust ouvre les fichiers choisis par le plugin `fs` (`FsExt::open`), qui accepte les deux. L'interface n'a aucune permission de ce plugin (vérifié par un test). **À vérifier sur le Mac** : sur iOS, le plugin ouvre un accès « security-scoped » à chaque fichier, qu'il faudrait refermer.

**Consultation** : la liste et le détail ne contiennent jamais de mot de passe. « Copier » est fait par Rust (`copy_password`) ; « Afficher » (`reveal_password`) envoie le mot de passe à l'interface, qui le masque de nouveau au bout de 30 s, ou dès qu'on change d'entrée. Recherche à chaque frappe (côté Rust, `pswd_app::search`), filtre par tag, `Ctrl+F` pour aller à la recherche. Toute lecture du coffre repousse son verrouillage ; si le coffre s'est verrouillé entre-temps, l'interface revient à la liste.

**Modification** : un formulaire unique pour ajouter et modifier (une note n'a ni identifiant, ni mot de passe, ni URL ; URLs une par ligne, tags séparés par des espaces ou des virgules). Pour le mot de passe : « Saisir » (le champ est vidé dès l'envoi), « Générer » (longueur 8 à 128, symboles ou non : **tiré en Rust à l'enregistrement**, il ne passe jamais par l'interface), et en modification « Inchangé » (par défaut) ou « Supprimer ». La suppression d'une entrée demande une confirmation dans l'écran. L'historique liste les versions précédentes (date, titre, identifiant, « mot de passe différent »), jamais les anciens mots de passe.

**Écran masqué** (§9) : sur Android, `FLAG_SECURE` est posé dans `MainActivity.kt` (`gen/android`, versionné ; à reporter si le projet Android est régénéré) : écran noir dans les applications récentes, captures et enregistrements refusés. Sur iOS, une vue neutre recouvre l'interface dès que l'application quitte le premier plan ; elle sera ajoutée au projet Xcode généré sur le Mac (sous-étape 6). Vérifié à la main sur appareil.

**Presse-papiers** : sur le bureau, `LazyClipboard` de `pswd-infra` (ouvert à la première copie, partagé avec la CLI). Sur mobile, un plugin maison implémente `SecureClipboard` avec les marqueurs du §9 ; le plugin officiel ne les pose pas.
- Android : classe `SecureClipboardPlugin.kt` dans le projet Android de l'application (`gen/android`), enregistrée par `android_clipboard.rs` (plugin Tauri déclaré dans la crate, sans crate séparée). Marqueur `android.content.extra.IS_SENSITIVE`. Chaque copie porte un identifiant aléatoire dans les extras de sa description, pour vérifier avant d'effacer que le presse-papiers la contient encore ; seul le minuteur de la dernière copie efface.
- **Limites Android** : depuis Android 10, seule l'application qui a le focus peut lire le presse-papiers. Si le délai expire pendant qu'on colle dans une autre application, l'effacement se fait sans vérifier : une copie faite ailleurs entre-temps est effacée aussi. Si le système tue l'application avant le délai, le minuteur disparaît : l'identifiant et l'échéance, gardés dans les préférences de l'application (rien de dérivé du mot de passe), font effacer la copie au lancement suivant. Le mot de passe traverse JNI dans une chaîne JSON, non effaçable, comme toute chaîne Java.
- iOS (sur le Mac) : l'enregistrement d'un plugin Swift passe par une fonction externe (`unsafe extern`), interdite dans `pswd-gui` (`#![forbid(unsafe_code)]`) : prévoir une petite crate dédiée. En attendant, la copie renvoie une erreur sur iOS.

**Verrouillage**
- Un seul coffre ouvert à la fois, gardé en Rust dans un `IdleLock` ; en ouvrir un autre verrouille le premier. Un coffre créé reste ouvert : sa clé de récupération est affichée une seule fois, puis oubliée par l'interface.
- Après `auto_lock_minutes` (10) sans action dans l'interface ; le minuteur vit en Rust. À l'échéance, la session est détruite et Rust envoie l'événement `vault-locked` : l'interface revient à la liste des coffres avec un message.
- Une touche ou un clic compte comme une action : l'interface le signale au backend (`current_vault`) au plus une fois toutes les 30 s. Si le coffre s'est verrouillé entre-temps, elle revient aussi à la liste.
- Mobile : application en arrière-plan plus de `background_lock_seconds` (60 ; 0 = immédiat) → verrouillage. Le délai permet d'aller coller un identifiant dans une autre application.
  - L'interface signale le départ et le retour (`visibilitychange` → `app_hidden`, `app_shown`). Au départ, Rust avance l'échéance de l'`IdleLock`, sans jamais la repousser ; une configuration illisible verrouille tout de suite.
  - Au retour, Rust compare l'horloge du système à l'heure du départ, car une application suspendue (iOS) gèle aussi son minuteur : délai dépassé ou heure reculée → verrouillage, et l'interface revient à la liste.
  - Sur le bureau, ces signaux sont ignorés : une fenêtre réduite n'est soumise qu'au verrouillage par inactivité.
- Bouton « Verrouiller » toujours visible. Pas de biométrie à cette étape (`EXTENSION(biometrie)`).

**Tests**
- `pswd-app` et le backend de `pswd-gui` : 100 % lignes et branches visées, comme `pswd-infra` ; exclusions justifiées (démarrage de Tauri, appel au plugin natif). Les commandes sont testées avec le moteur factice de Tauri (`tauri::test`) et la vraie configuration, capacités comprises.
- Sous Windows, le manifeste de l'application (contrôles communs v6) est lié à tous les exécutables par `build.rs`, sinon les tests ne démarrent pas.
- Kotlin et Swift : réduits au minimum, testés à la main sur un appareil réel.
- TypeScript : Vitest, 100 % des lignes pour la logique (`.ts`) ; composants Svelte testés par comportement avec Testing Library, sans seuil.
- `svelte-check` et ESLint strict sans avertissement ; `package-lock.json` versionné et `npm audit` sans faille, comme `cargo deny`.
- Pas de test de bout en bout pour l'instant (`tauri-driver` : Windows et Linux seulement) ; un test de démarrage possible à la sous-étape 7.
- Builds mobiles lancés à la main : Android à la fin de chaque sous-étape qui touche au mobile, iOS (sur le Mac) aux sous-étapes 2, 6 et 7.
