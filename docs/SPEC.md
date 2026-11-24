# SPEC — PSWD Master

Référence de conception. Toute décision qui change ce document doit y être reportée.

## 1. Objectif

Gestionnaire de mots de passe personnel, local-first, pour remplacer un service tiers (NordPass).
L'utilisateur reste maître de ses données : elles vivent sur ses appareils, sur ses disques de sauvegarde et, s'il le souhaite, sur son propre serveur. Aucun service tiers n'intervient.
Le coffre est chiffré de bout en bout : le serveur ne voit jamais de données en clair ni de clé.

## 2. Feuille de route

**Étape 1 — local complet** (terminée) : le cœur de l'application (`pswd-core`, `pswd-infra`, `pswd-app`).
- Plusieurs coffres indépendants et entièrement locaux (un mot de passe maître et une clé par coffre).
- Entrées `Login` et `Note`, tags, historique des versions.
- Générateur de mots de passe, copie sécurisée dans le presse-papiers.
- Changement de mot de passe maître, clé de récupération.
- Import CSV (format §8). Pas d'export en clair : il exposerait tous les mots de passe dans un fichier, sans besoin réel (les sauvegardes, elles, restent chiffrées).
- Sauvegarde et restauration sur disque externe, suppression d'un coffre (§7).

**Étape 2 — interfaces graphiques** (terminée sur le bureau et Android ; iOS reporté, en attente d'un Mac, §12) : application **PSWD Master** pour le bureau (Windows, Linux, macOS) et le mobile (Android, iOS), un seul projet Tauri 2. Chaque appareil est autonome et entièrement local, avec toutes les fonctionnalités de l'étape 1. Sous-étapes, chacune validée et commitée :
1. `pswd-app` : cas d'usage partagés, indépendants de l'interface.
2. Squelette Tauri (`pswd-gui`) : CSP et capacités, premier build Android et iOS ; liste des coffres, création, déverrouillage, verrouillage.
3. Consultation : liste, recherche, tags, détail, copier, afficher. Ouvrir une adresse dans le navigateur du système (ajouté le 2026-11-10, #16 des tests d'intégration). Reporté : copier l'identifiant.
4. Modification : ajout, édition, suppression, historique, générateur.
5. Gestion des coffres : mot de passe maître, récupération, import CSV, sauvegarde et restauration, suppression, réglages.
6. Mobile : presse-papiers sensible, verrouillage en arrière-plan, écran masqué, fichiers `.pswd`, ergonomie tactile.
7. Installateurs, documentation, mutants.

Reste à faire sur le Mac : build iOS, plugin Swift du presse-papiers, écran neutre, accès « security-scoped » (§12), essai de l'ouverture des adresses dans le navigateur (plugin `opener`, §9).

**Étape 3 — serveur maison** (§11) : `pswd-server`, comptes, coffres privés et communs, synchro entre appareils. Sous-étapes par fonctionnalité, du serveur jusqu'à l'écran, chacune validée et commitée :
1. Serveur minimal : crate `pswd-server`, base SQLite créée au démarrage, lecture et publication conditionnelle d'un coffre, 15 versions conservées ; HTTP simple, sans comptes, réservé aux tests.
2. TLS et client : certificat, `rustls`, adaptateur `RemoteStore` HTTP dans `pswd-infra` avec vérification de l'empreinte, testé contre un `pswd-server` local.
3. Comptes et appareils : jetons, clés de connexion, administrateur initial, `reset-admin`, révocation, membres de chaque coffre.
4. Synchro côté client (`pswd-infra`, `pswd-app`) : connexion, déconnexion, ajout d'un appareil, échange avec le serveur, états des coffres, téléchargement initial. Le modèle des états a été simplifié le 2026-10-31 (§11) : publication automatique de tout coffre local, synchro sans mot de passe quand un seul côté a changé.
5. Synchro dans PSWD Master : connexion, état de synchro, bouton « Synchroniser » (build Android repoussé, à lancer à la demande).
6. Coffres communs : inviter, accepter ou refuser, annuler une invitation, quitter (serveur et PSWD Master) ; état « retiré » d'un coffre quitté. Trois blocs : serveur, client (`pswd-infra`, `pswd-app`), interface.
7. Administration : panneau de PSWD Master (membres, appareils, rôles, clés en attente, suppression, rattachement, versions, adresse publique, sauvegarde du serveur) ; suppression d'un coffre privé sur le serveur par son propriétaire. Trois blocs : serveur, client (`pswd-infra`, `pswd-app`), interface.
8. Installation réelle : paquet `.deb`, `backup` / `restore` du serveur, VM puis PC réel, documentation, mutants.

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
| `pswd-gui` | Application Tauri 2 **PSWD Master** : backend Rust (commandes, session, verrouillage) et interface Svelte 5 + TypeScript (§12) | `#![forbid(unsafe_code)]` |

Étape 3 : `pswd-server` (exécutable du serveur, `#![forbid(unsafe_code)]`, §11) et l'adaptateur `RemoteStore` HTTP dans `pswd-infra`. Dépendances : serveur `axum` + `tokio`, `clap` (sous-commandes), `axum-server` (HTTPS), `rustls` (moteur `ring`, ni OpenSSL ni `aws-lc-rs`), `rcgen` (certificat), `rusqlite` avec SQLite intégré (`bundled`) ; client `reqwest` en mode bloquant (synchrone comme le port), avec une configuration `rustls` fournie toute prête et un vérificateur d'empreinte maison. `ureq` a été écarté : sa configuration TLS n'accepte pas de vérificateur personnalisé. Pour les tests : `tower`, `http-body-util`, `futures-util`. Licences ajoutées : ISC (`ring`, `rustls-webpki`, `untrusted`), et CDLA-Permissive-2.0 pour la seule crate `webpki-root-certs`.

**Ports (définis dans `pswd-core`)**
- `LocalStore` : `load() -> Option<bytes>` (`None` : pas de coffre local), `save_atomic(bytes)` (fichier temporaire + renommage).
- `RemoteStore` (étape 3) : `fetch() -> Option<(bytes, version)>` (`None` : distant vide ; une erreur vaut « hors ligne », l'adaptateur borne lui-même l'attente), `push(bytes, expected_version)` (`Conflict` si le distant a bougé ; `expected = None` exige un distant vide).
- `SecureClipboard` : `copy_secret(value, ttl)`.
- Les erreurs des adaptateurs sont des `PortError` (un message, jamais de secret). Deux sortes se distinguent : coffre retiré par le distant (`PortError::removed`) et distant injoignable (`PortError::unreachable`, aucune adresse n'a répondu).

**Session** (`pswd-core`) : coffre ouvert, déverrouillé par mot de passe ou clé de récupération (`Credentials`, extensible au fichier-clé). Toute modification est enregistrée localement et marque le coffre comme ayant des modifications non publiées. Avant chaque écriture (modification, changement de mot de passe, synchro), la copie locale est relue : si un autre programme l'a réécrite depuis la dernière lecture ou écriture de la session, elle est fusionnée (§6) ; sinon elle ne l'est pas, car refusionner sa propre écriture, plus ancienne, pourrait annuler à date égale la dernière modification. Un mot de passe changé ailleurs est remplacé par le nouveau. Une copie locale disparue ou inutilisable n'est jamais remplacée : l'écriture est refusée. La synchro (§7) est une opération distincte qui reçoit le distant en paramètre : une session n'a besoin d'aucun distant. Les magasins en mémoire la rendent testable ; PSWD Master l'utilise telle quelle.

La crypto n'est **pas** un port : déterministe, sans E/S, identique partout.

**Dépendances** : `argon2`, `chacha20poly1305`, `hkdf` + `sha2`, `getrandom`, `rand` (générateur de mots de passe), `zeroize`, `serde` + `serde_json`, `csv`, `uuid`, `etcetera` (dossiers standard, licence MIT/Apache, contrairement à `directories` qui tire du MPL-2.0), `toml`, `arboard` (presse-papiers du bureau), `tauri` 2 avec les plugins `dialog` (fenêtres système) et `fs` (fichiers choisis sur mobile). Tests : `proptest`, `tempfile`. Côté interface (npm) : `svelte` 5, `vite`, `typescript`, `@tauri-apps/api` et `plugin-dialog`, la police `@fontsource-variable/atkinson-hyperlegible-next`, `marked` (aide de l'application, §12) ; pour les tests et contrôles, `vitest`, Testing Library, `svelte-check`, ESLint ; aucune bibliothèque de composants graphiques.

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

**Clé du coffre** : 32 octets aléatoires, générée à la création du coffre, chiffre le payload. Elle n'est jamais stockée en clair ; elle est « enveloppée » par une ou plusieurs clés d'enveloppe :

| `kind` | Enveloppe | Clé d'enveloppe |
|---|---|---|
| 1 | Mot de passe | Argon2id(mot de passe, sel) ; contient `m_cost`, `t_cost`, `p`, `salt` (16 o) |
| 2 | Récupération | HKDF-SHA256(clé de récupération, info = `"pswd-recovery-v1"`) |
| 3 | *Réservé* : mot de passe + fichier-clé | — |

Chaque enveloppe : `kind u8 | paramètres propres | nonce 24 o | XChaCha20-Poly1305(clé d'enveloppe, vault_key)` (48 o). `kind` et les paramètres forment l'AAD de l'enveloppe. Paramètres du kind 1 : `m_cost u32 | t_cost u32 | p u32 | salt 16 o` ; le kind 2 n'en a pas.

En version 1, l'en-tête contient exactement une enveloppe 1 et une enveloppe 2, dans n'importe quel ordre. Un `kind` inconnu est refusé (sa taille est inconnue) ; une enveloppe absente ou en double rend le fichier mal formé. Limiter à une enveloppe mot de passe évite qu'un fichier piégé impose des centaines de calculs Argon2. Le fichier est entièrement découpé et validé avant toute dérivation de clé.

- Paramètres Argon2id par défaut : 64 MiB, 3 itérations, parallélisme 1 (compatible mobile). Ils sont lus depuis le fichier, donc modifiables sans migration. Bornes acceptées (contre un fichier malveillant) : mémoire 8 MiB – 1 GiB, 1 à 10 itérations, parallélisme 1 à 8.
- Aléa (clés, sels, nonces) : `getrandom` (générateur du système).
- Clé de récupération : 32 octets aléatoires, affichés **une seule fois** à la création du coffre, encodés en Base32 Crockford par groupes de 4 caractères : 52 caractères en 13 groupes séparés par `-`. À la saisie, seule l'écriture affichée est acceptée, tirets compris, en majuscules ou en minuscules ; les espaces autour (copier-coller) sont ignorés. Aucune autre tolérance : ni espaces à la place des tirets, ni `O` lu `0`. Les 4 bits de complément doivent être nuls (une seule écriture par clé). Deux messages : « mal saisie » si la forme est fausse, « incorrecte » si elle est bonne mais que la clé n'ouvre pas le coffre (ou ne peut être aucune clé).
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

**En-tête** : les enveloppes retenues sont celles du fichier dont `keys_updated_at` est le plus récent (propage un changement de mot de passe) ; à date égale, les octets de l'en-tête départagent, pour que tous les appareils convergent. Celles du fichier distant ne sont adoptées qu'après déchiffrement de son payload (l'en-tête est authentifié comme AAD). La clé du coffre est identique des deux côtés, puisque le changement de mot de passe maître et la récupération ne la changent pas ; un fichier chiffré par une autre clé est refusé comme altéré, un coffre d'un autre identifiant comme « autre coffre ».

**Changement de mot de passe** (changement de mot de passe maître, récupération) : nouvelle enveloppe 1 avec un nouveau sel ; l'enveloppe 2 est conservée. `keys_updated_at` = max(maintenant, valeur actuelle + 1 ms), pour que le changement l'emporte même si l'horloge a reculé.

**Limite acceptée** : `updated_at` dépend de l'horloge de l'appareil ; une horloge très décalée peut faire gagner la mauvaise version (la perdante reste dans l'historique).

**Restaurer n'est pas fusionner** : une ancienne copie (sauvegarde, version conservée par le serveur) n'est jamais fusionnée dans le coffre actuel. Ses entrées supprimées depuis seraient aussitôt ré-effacées par les tombstones, plus récentes. Une restauration crée toujours un nouveau coffre.

## 7. Stockage, sauvegarde et synchro

**Emplacements** (dossiers standard de l'OS, crate `etcetera` : `AppData\Roaming` sous Windows, `~/.config` et `~/.local/share` ailleurs)
- Config : `<config_dir>/pswd/config.toml`, contenant les coffres nommés (`[vaults.<nom>]` : chemin local), `auto_lock_minutes = 10`, `clipboard_clear_seconds = 20`, `background_lock_seconds = 60` (mobile, §12) et `theme = "auto"` (`auto`, `light` ou `dark`, réglé par PSWD Master).
- Données : un dossier par coffre, `<data_dir>/pswd/vaults/<identifiant>/` (UUID v4 tiré à la création, sans tirets), contenant `vault.enc` (`FileStore`). Plusieurs coffres coexistent ; aucun ne remplace un autre. Le nom du coffre n'apparaît pas dans le chemin : il peut contenir n'importe quel caractère et changer sans déplacer de fichier (décidé le 2026-11-10, #15 des tests d'intégration ; les coffres créés avant restent sous `vaults/<nom>/` et fonctionnent tels quels, puisque `config.toml` garde le chemin de chaque coffre).
- Nom d'un coffre : texte libre, espaces et accents compris (`pswd_core::is_valid_vault_name`) : ni vide, ni entouré d'espaces, sans caractère de contrôle, 32 caractères au plus (64 jusqu'au 2026-11-11, jugé trop long aux tests V1.2 ; même codés sur 4 octets, ils tiennent dans les 255 octets d'une sauvegarde). Les noms sont comparés à l'identique : « Été », « Ete », « Banque » et « banque » sont quatre noms distincts. PSWD Master retire les espaces au début et à la fin de la saisie et la met sous forme Unicode NFC (deux noms identiques à l'écran le sont aussi pour l'application). Un nom pris est refusé tant que le coffre existe. Il n'y a pas de coffre par défaut : chaque commande nomme son coffre.
- Les restes de la v1 (dossiers `repo.git/`, jetons `pswd` du trousseau) ne sont plus utilisés et peuvent être supprimés à la main ; les entrées `url` et `default_vault` de `config.toml` doivent être retirées.

**Sauvegarde** : un seul format sur tous les appareils, un fichier `.pswd` (sur mobile, une sauvegarde ne peut pas être un dossier). Il sert aussi à passer ses coffres d'un appareil à l'autre.
- Contenu : les coffres locaux cochés dans l'interface, copiés bruts, toujours chiffrés : aucun mot de passe demandé, rien de lisible sur le disque. Nom proposé : `pswd-sauvegarde_<AAAA-MM-JJ_HH-MM-SS>.pswd` (UTC). Les noms des coffres sont rangés dans le fichier : un sélecteur de fichiers mobile ne donne pas toujours le nom du fichier, et celui-ci a pu être renommé.
- L'emplacement est choisi par la fenêtre système d'enregistrement (§12).
- Le fichier est sur le disque (`sync_all`) avant la fin de la commande. Supprimer les anciennes sauvegardes est laissé à l'utilisateur.
- Format (`pswd-core`, binaire, little-endian) :

```
magic          "PSWDSAVE"        8 o
format_version u8                (1)
created_at     i64 (ms UTC)      date de la sauvegarde
vault_count    u16               1 000 au plus
pour chaque coffre, noms triés et distincts :
  name_len     u8
  name         UTF-8             nom de coffre valide (§7)
  file_len     u32               64 Mio au plus
  file         vault.enc         vérifié (structure, version) à la lecture
```

- Le fichier est lu en entier et chaque coffre vérifié avant d'en restaurer un ; rien après le dernier coffre. Une sauvegarde n'a qu'une écriture (noms triés) : le fuzzing vérifie que tout fichier accepté se réencode à l'identique.

**Restauration**
- L'utilisateur choisit un fichier `.pswd`, puis les coffres à restaurer parmi ceux qu'il contient (§12).
- Le fichier est vérifié (structure, version) avant toute écriture ; un fichier altéré à l'intérieur du chiffrement ne se voit qu'à l'ouverture.
- Le coffre est restauré comme **nouveau coffre local** : sous son nom d'origine s'il est libre, sinon sous un nom distinct (`<nom>-sauvegarde-<AAAA-MM-JJ>`, refusé s'il est pris à son tour). Rien n'est jamais écrasé ni fusionné (§6).
- Il s'ouvre avec le mot de passe en vigueur à la date de la sauvegarde.

**Suppression d'un coffre**
- Le nom du coffre doit être retapé ; un rappel propose de faire une sauvegarde avant.
- Seuls `vault.enc` et un éventuel `vault.enc.tmp` sont supprimés, puis le dossier s'il est vide : le chemin vient de `config.toml`, modifiable à la main, donc un dossier n'est jamais effacé en entier.
- Fichiers d'abord, configuration ensuite : un échec de suppression ne change rien.
- Le nom redevient libre.
- À l'étape 3, un coffre privé publié propose « Supprimer partout » ou « Retirer de cet appareil » ; un coffre commun se quitte, et son dernier membre le supprime (§11).

**Synchro** (algorithme de `pswd-core`, utilisé à partir de l'étape 3)
1. `fetch`. Distant injoignable : état « hors ligne ».
2. Distant présent : déchiffrement, fusion, écriture locale si la copie locale est en retard. Si le distant est à jour, fin.
3. Publication avec la version récupérée. Rejetée (le distant a bougé) : retour en 1, 3 tentatives max.

Précisions :
- Copie locale et distant ne sont réécrits que s'ils sont en retard sur la fusion : synchroniser un coffre à jour n'écrit rien, et une modification sans effet n'écrit rien.
- Si l'écriture locale d'une modification échoue, le coffre en mémoire revient à son état précédent.
- Un fichier distant inutilisable (altéré, autre coffre, version inconnue) n'est **jamais écrasé** : état « refusé », le coffre local reste utilisable.
- États de synchro : à jour, hors ligne (avec le message de l'adaptateur), conflit persistant après 3 tentatives, distant refusé.
- **Sans mot de passe** : chaque coffre lié au serveur retient son dernier état commun avec lui (`[vaults.<nom>.remote.synced]` : `version` du serveur et `file_hash`, empreinte SHA-256 de la copie locale à ce moment). Un échange (bouton « Synchroniser » de la liste, échange automatique ci-dessous) compare la copie locale à cette empreinte et la version du serveur à celle retenue : si seule la copie locale a changé, elle est envoyée telle quelle (`If-Match` sur la version retenue) ; si seul le serveur a bougé, sa copie, vérifiée par `VaultFile::check_format`, remplace la copie locale. C'est sans perte : chaque version du serveur et chaque écriture locale contiennent déjà la précédente. Si les deux ont changé, ou si l'état commun est inconnu (copies différentes, coffre lié avant cette règle), il faut déverrouiller le coffre pour fusionner. Une synchro avec clé réussie retient l'état commun (version vue par l'adaptateur, copie locale écrite par la session, `Session::saved_file`).
- La synchro avec fusion exige la clé du coffre, donc un coffre déverrouillé. Moments : au déverrouillage (création et récupération comprises), après chaque modification, à la fermeture s'il reste des modifications non publiées et à la demande (bouton « Synchroniser »). Pas de synchro au verrouillage par inactivité : la clé est déjà effacée. La modification est d'abord enregistrée localement ; la synchro se fait ensuite en arrière-plan, sans jamais figer l'interface.
- **Échange automatique** : l'échange sans mot de passe se lance seul tant que la liste des coffres est affichée et l'application au premier plan :
  - au démarrage et à chaque retour sur la liste ;
  - quand la fenêtre ou l'application revient au premier plan ;
  - toutes les 5 s (30 s jusqu'au 2026-11-11 : trop lent à l'usage, tests V1.2).

  Il ne se lance pas si un échange ou une synchro est déjà en cours (pas de file d'attente), ni en arrière-plan (batterie). Il est muet : la liste et les pastilles se mettent à jour, sans message ; seul un nouveau coffre reçu donne une notification courte (une invitation apparaît en haut de la liste, sans notification). Le bouton « Synchroniser » garde le compte rendu. Coût : sans changement, deux petites requêtes (liste des coffres, informations du serveur) ; un coffre n'est téléchargé que s'il a changé. Plusieurs appareils et comptes à 5 s ne pèsent rien pour le serveur. Conséquence : l'état « à mettre à jour » (§11) est corrigé seul en quelques secondes ; une pastille orange qui reste signale une action nécessaire.
- **Coffre ouvert** (PSWD Master) : toutes les 5 s, tant que l'application est au premier plan, la version du serveur (liste des coffres) est comparée à celle du dernier état commun, sans toucher à la session ; si elle a changé, synchro avec fusion. Cette vérification et cette synchro ne comptent pas comme une activité : elles ne repoussent pas le verrouillage par inactivité. Rien pour un coffre supprimé du serveur ou absent de la liste, ni si un échange ou une synchro est déjà en cours. Plus généralement, aucune synchro ne compte comme une activité (`IdleLock::quietly`) : seules les actions de l'utilisateur repoussent le verrouillage.
- **Mot de passe changé sur un autre appareil** (PSWD Master) : le déverrouillage se fait sur la copie locale. L'échange automatique et la vérification du coffre ouvert l'amènent à jour en 5 s au plus ; seule une copie en retard (appareil éteint ou hors ligne, échange pas encore fini) s'ouvre encore avec l'ancien mot de passe et refuse le nouveau. Ouverte avec l'ancien, la synchro adopte aussitôt le nouvel en-tête (§6), et une notification l'indique : « Mot de passe maître changé sur un autre appareil. » (`passwordChanged` du compte rendu de synchro, seulement quand la copie synchronisée est rendue au coffre ouvert). Pas de nouvel essai automatique après un refus : la fenêtre est de quelques secondes, retaper le mot de passe suffit (décidé le 2026-11-07).
- PSWD Master synchronise une **copie** de la session (`Session` est `Clone`) : pendant l'échange, l'interface lit et modifie le coffre ouvert. La copie synchronisée ne remplace la session que si le coffre n'a été ni fermé ni modifié entre-temps ; sinon la modification a déjà prévu une autre synchro. Une seule synchro ou un seul échange avec le serveur à la fois : deux fils ne réservent jamais le même coffre et n'écrivent jamais la configuration ensemble. Une copie garde la clé du coffre jusqu'à la fin de son échange (2 min au plus), même si le coffre est verrouillé entre-temps.

## 8. Création, recherche, générateur et import

- **Création d'un coffre** : mot de passe maître fort (§9), affichage unique de la clé de récupération. Refusée si le nom est déjà pris : aucune opération n'écrase un coffre existant. Le nom est vérifié avant tout ; un essai raté ne laisse rien derrière lui (dossier du coffre supprimé s'il n'existait pas avant, configuration inchangée).
- **Presse-papiers** (bureau) ouvert à la première copie : une machine sans presse-papiers peut utiliser tout le reste.
- **Recherche** : sans tenir compte de la casse, sur le titre, l'identifiant, les URLs et les tags ; les tombstones sont ignorés.
- **Générateur** : 24 caractères par défaut (8 à 24), majuscules, minuscules, chiffres et symboles `!#$%&()*+,-./:;<=>?@[]^_{|}~` (ni espace, ni guillemets, ni antislash, ni accent grave) ; au moins un caractère de chaque classe active, obtenu par rejet des candidats incomplets pour que tous les mots de passe valides restent équiprobables. `pswd-core` reçoit le générateur aléatoire en paramètre, contraint par le trait `CryptoRng` ; `pswd-app` (`generate`) fournit un `StdRng` initialisé par `SysRng` (aléa du système, `rand` 0.10) : si l'aléa manque, une erreur est renvoyée, jamais de panique.
- **Affichage** : mot de passe masqué par défaut ; le mot de passe maître n'est jamais copié, et n'est affiché que pendant sa saisie, à la demande de l'utilisateur (icône en forme d'œil).

**Format CSV d'import** : tout le fichier est validé avant d'ajouter quoi que ce soit, puis l'application rappelle de le supprimer.

```csv
Titre / Title;Type / Kind;Nom d'utilisateur / Username;Mot de passe / Password;Urls;Notes;Tags
GitHub;login;arthur;xxxx;https://github.com;;"dev;perso"
Code Wi-Fi;note;;;;"Box: ABCD-1234";maison
```
Modèle à remplir : `docs/template_import_pswd.csv` (sa ligne d'exemple est à remplacer), embarqué dans PSWD Master (`include_bytes!`) : l'écran d'import propose « Télécharger le modèle », que Rust écrit dans le fichier choisi par la fenêtre système (commande `csv_template_to_file`). Colonnes séparées par `;`, comme l'enregistre Excel en français. Le type (`kind`) vaut `login` ou `note` ; `urls` et `tags` séparés par `;`, donc la cellule entre guillemets dès qu'elle en contient plusieurs (Excel le fait seul). RFC 4180 pour le reste (guillemets, retours à la ligne dans les champs). Une ligne invalide fait échouer l'import avec son numéro de ligne ; rien n'est importé partiellement.

- **Avertissement** : l'écran d'import rappelle, avant le choix du fichier, qu'il contient les mots de passe en clair et qu'il faut le supprimer (corbeille comprise) après l'import ; la confirmation de l'import le redit.

- Listes (`urls`, `tags`) : `\;` et `\\` désignent un `;` et un `\` littéraux ; un `\` isolé est gardé tel quel ; les éléments vides sont ignorés.
- Une cellule vide est un champ absent (`username`, `password`, `notes`). `kind` est lu sans tenir compte de la casse. Le BOM UTF-8 (Excel) est ignoré ; fins de ligne `\r\n`, `\n` ou `\r`.
- L'en-tête doit être exactement celui ci-dessus. Les numéros de ligne sont ceux du fichier, y compris dans les champs sur plusieurs lignes ; ils sont calculés par `pswd-core` (ceux de la crate `csv` sont faux avec `\r\n` et les lignes vides). Les messages d'erreur ne citent jamais le contenu.
- L'application n'écrit jamais de CSV. Les tests et le fuzzing vérifient l'aller-retour avec un écrivain réservé aux tests (`#[cfg(any(test, fuzzing))]`) : tout CSV accepté se relit à l'identique.
- Limite acceptée : les buffers internes de la crate `csv` ne sont pas effacés ; le fichier CSV est de toute façon en clair.

## 9. Sécurité

- Secrets (`password`, clés, mot de passe maître, JSON déchiffré) dans des types effacés à la libération (`Zeroizing` de `zeroize`, `Secret` de `pswd-core`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- **Presse-papiers** : seulement les mots de passe d'entrées ; effacement après 20 s, ou dès la fermeture du programme si elle survient avant, uniquement s'il contient encore la valeur copiée (sauf sur Android quand il est illisible, §12) ; seul le minuteur de la dernière copie efface. Le contenu doit être exclu de l'historique dès la copie :
  - Windows : formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`, `CanUploadToCloudClipboard = 0` ;
  - Linux : type MIME `x-kde-passwordManagerHint = secret` (partiel : avertissement affiché) ;
  - Android 13+ : `EXTRA_IS_SENSITIVE` (§12) ; iOS (à faire sur le Mac) : `localOnly` + `expirationDate`.
  - **Fermeture sur le bureau** : Tauri termine le programme sans libérer son état, donc sans exécuter les destructeurs. L'effacement est explicite (`SecureClipboard::clear_now`, appelé sur `RunEvent::Exit`). Sous Linux, il évite aussi de confier la copie au gestionnaire de presse-papiers, qui reprend le contenu d'un programme qui se ferme. Limite : un programme tué (gestionnaire des tâches, plantage, coupure) n'efface rien, et son minuteur disparaît avec lui.
- **Mot de passe maître** : tout nouveau mot de passe maître (création, changement de mot de passe, récupération) doit avoir au moins 12 caractères (comptés en caractères, pas en octets) et mélanger au moins 3 types parmi minuscules, majuscules, chiffres et symboles (tout autre caractère : ponctuation, espace, lettre sans casse). La règle vit dans `pswd-app` (`check_master_password`) ; l'interface la refait avant l'envoi (`validation.ts`) et l'affiche sous le champ. Un mot de passe déjà en place n'est jamais revérifié : il ouvre toujours son coffre.
- Mot de passe maître **et** clé de récupération perdus = coffre perdu. Aucune récupération possible côté serveur.
- **Anciennes copies** : après un changement de mot de passe maître ou une récupération, l'ancien mot de passe ouvre toujours les copies antérieures du fichier (sauvegardes externes, versions conservées par le serveur). Comme ces opérations ne changent pas la clé du coffre, qui a gardé une ancienne copie **et** l'ancien mot de passe obtient cette clé, qui déchiffre aussi les versions suivantes s'il se les procure (sauvegarde `.pswd`, accès à la base du serveur) ; c'est le serveur qui l'en empêche (membre retiré, appareil révoqué). **Sujet à trancher plus tard** (noté le 2026-11-07) : changer la clé du coffre avec le mot de passe (`key-rotation`, §10) ; conséquence à prévoir : un appareil resté sur l'ancienne clé ne peut plus fusionner (fichier « altéré », §6) et doit reprendre la copie du serveur, en perdant ses modifications hors ligne.
- **Interface graphique** : JavaScript ne peut pas effacer une chaîne de la mémoire ; le moins de secrets possible traversent vers la webview (§12).
  - Jamais côté JS : clé du coffre, coffre déchiffré, mot de passe copié (la copie est faite par Rust).
  - Côté JS seulement sur demande explicite : mot de passe affiché (« Afficher », masqué à nouveau après 30 s), mot de passe généré affiché.
  - **Limite acceptée** : les saisies (mot de passe maître, mot de passe d'une entrée) passent par un champ HTML, donc par JS ; le champ est vidé dès l'envoi.
  - CSP stricte (ressources de l'application seulement, ni script externe, ni `eval`, ni réseau) ; capacités Tauri limitées à nos commandes (aucun accès fichiers ou shell depuis JS) ; les adresses d'une entrée s'ouvrent dans le navigateur du système, jamais dans la webview : un clic appelle la commande `open_url`, qui n'accepte qu'une adresse `http` ou `https` avec un nom d'hôte et sans identifiant (`https://moi@site` déguise le vrai site) ; une adresse sans protocole (`exemple.fr`) s'ouvre en `https`. Le backend seul appelle le plugin `opener` (module `links.rs`) ; l'interface n'a aucune de ses permissions (vérifié par un test). Le plugin est fixé à la version 2.7.2.
- **Mobile** : écran masqué dans les applications récentes et captures interdites (Android `FLAG_SECURE`, écran neutre sur iOS).

## 10. Points d'extension à préparer

Chaque point est marqué dans le code par un commentaire `// EXTENSION(<nom>): …` à l'endroit exact où il se branchera, dès que cet endroit existe.

| Nom | Préparation |
|---|---|
| `keyfile` | `kind = 3` réservé dans les enveloppes ; dérivation de la clé d'enveloppe isolée pour accepter un second secret ; port de déverrouillage générique. |
| `key-rotation` | Les enveloppes sont indépendantes du payload ; le changement de mot de passe maître pourra générer une nouvelle clé de coffre et rechiffrer. À trancher : rotation à chaque changement de mot de passe ou sur demande, et règle de fusion entre deux clés (§9, « Anciennes copies »). |
| `mobile` | `pswd-core` sans E/S ; port `SecureClipboard` réimplémenté par un plugin Tauri maison : Kotlin fait, Swift à faire sur le Mac (§12). |
| `biometrie` | Déverrouillage par empreinte ou Face ID : une clé rangée dans le stockage sécurisé du téléphone (Keystore, Keychain), à concevoir à part. |
| `anglais` | Textes de l'interface regroupés par langue (`src/lib/i18n/fr.ts`) ; `en.ts` devra avoir les mêmes clés (vérifié par TypeScript). Les erreurs Rust remontent à l'interface sous forme de codes, traduits côté interface. |
| `remote-transport` | `RemoteStore` abstrait : l'adaptateur HTTP de `pswd-server` s'y branche (étape 3). |
| `acces-exterieur` | Accès au serveur hors de la maison (prochaine version, §11). Le serveur, l'API (`PUT /v1/admin/settings`) et le client gèrent déjà l'adresse publique ; dans le panneau d'administration, le champ est affiché grisé avec la mention « Bientôt disponible » (`AdminServer.svelte`), à rendre modifiable. |
| `partage-par-membre` | Plusieurs enveloppes possibles par coffre (une par membre, à clé publique) : ajouter ou retirer un membre sans partager de mot de passe. Les coffres communs de l'étape 3 utilisent un mot de passe partagé. |
| `dossier-reseau` | Synchro par un partage réseau (NAS) sans `pswd-server`. Un dossier ne sait pas refuser une écriture concurrente : il faudrait un fichier par appareil, fusionnés à la lecture. |
| `multi-serveurs` | Un appareil connecté à plusieurs serveurs : chaque coffre est rattaché à un serveur. |
| `cert-rotation` | Changer le certificat du serveur (clé privée volée) : les appareils devraient retenir la nouvelle empreinte sans se reconnecter un par un. |

## 11. Serveur maison (étape 3)

**Principe** : `pswd-server` est un exécutable Rust unique, sans dépendance à installer, lancé sur une machine de l'utilisateur. Cible : un PC sous **Ubuntu Server 26.04 LTS** (x86_64), en service `systemd` ; essayé d'abord dans une machine virtuelle Ubuntu Server 26.04 (VirtualBox ou VMware, réseau en pont) avant le vrai déploiement. C'est une passerelle : il stocke des fichiers `vault.enc` opaques et ne détient aucune clé. Toute la gestion se fait depuis PSWD Master.

**Installation** : un paquet `.deb` (`cargo-deb`), installé par `sudo apt install ./pswd-server_<version>_amd64.deb`, sans aucune étape manuelle. Il installe `/usr/bin/pswd-server`, crée l'utilisateur système `pswd-server` (sans connexion ; le serveur ne tourne jamais en `root`) et le dossier `/var/lib/pswd-server/` (base et certificat, accessibles à ce seul utilisateur), affiche l'adresse (première adresse de `hostname -I`, port de la configuration), la clé administrateur et un rappel : le port se change dans la configuration avant de connecter le premier appareil (guide d'utilisation de l'application), puis installe et démarre le service. À la première installation, la base, le certificat et la clé sont créés par `reset-admin` (lancé en `pswd-server` par `runuser`) **avant** le démarrage du service, qui ne doit pas créer le certificat en même temps. Configuration : `/etc/pswd-server/config.toml` (`port`, 8443 par défaut ; fichier absent : valeurs par défaut) ; le service écoute sur toutes les interfaces à ce port. Une mise à jour (nouveau `.deb`) et `apt remove` conservent les données ; seul `apt purge` les efface (avec l'utilisateur système). Le service `systemd` (`debian/pswd-server.service`) tourne en `pswd-server`, n'écrit que dans `/var/lib/pswd-server` (`ProtectSystem=strict`) et n'a aucun privilège (`CapabilityBoundingSet=` vide, `NoNewPrivileges`). Fichiers du paquet dans `crates/pswd-server/debian/`, toujours en fins de ligne LF (`.gitattributes`). Guide pas à pas : `docs/SERVEUR.md`.

**Variante Docker** (décidée le 2026-11-14, revue le 2026-11-16) : pour les machines où le paquet `.deb` ne s'installe pas (autre Linux que Debian/Ubuntu, Windows). `crates/pswd-server/docker/` (`Dockerfile` en deux temps, compilation dans `rust:1-slim-trixie` puis exécution dans `debian:trixie-slim` ; contexte de construction à la racine du dépôt, `.dockerignore` à la racine ; `compose.yaml`, seul fichier copié sur le serveur). L'image est construite sur l'ordinateur de développement, transférée par `docker save` / `docker load` : le serveur n'a ni le dépôt ni Rust, et le conteneur n'est jamais téléchargé d'un registre (`pull_policy: never`). Utilisateur fixe 10001 (jamais `root`) ; données et sauvegardes dans deux volumes nommés (`donnees` → `/var/lib/pswd-server`, `sauvegardes` → `/sauvegardes`), identiques sous Linux et Windows et créés avec le propriétaire de l'image : aucun dossier à préparer ; une sauvegarde sort par `docker compose cp`, et y entre de même pour `restore`. Pas de fichier de configuration (port 8443 dans le conteneur, publié sur le port 8543 de la machine pour ne pas gêner le paquet `.deb` ; à changer dans `compose.yaml`). Mêmes protections que le service : système de fichiers en lecture seule hors des données, aucune capacité, `no-new-privileges`, redémarrage automatique. Premier démarrage : `reset-admin` lancé à part avant `up`, comme le paquet. **Moteur Docker en root** (compromis accepté le 2026-11-16) : le mode rootless, essayé le 2026-11-15, est abandonné, trop lourd à installer et propre à Linux ; la variante sert à faire tourner l'image partout (tout Linux, Docker Desktop sous Windows 10/11, Docker Engine dans WSL 2 sous Windows Server). La sécurité repose sur le conteneur bridé ; sous Linux, `sudo docker` seulement, jamais de groupe `docker` (équivalent à root sans mot de passe). Un port publié par Docker en root n'est pas filtré par `ufw` (guide).

**Stockage** : une base SQLite (`/var/lib/pswd-server/pswd.db`, moteur intégré à l'exécutable par `rusqlite` `bundled`), créée au premier démarrage. Tout y est, contenu des coffres compris (colonne `BLOB`) : une publication (vérification de la version, nouvelle version, retrait de la plus ancienne au-delà de 15) est une seule transaction, qu'une coupure de courant ne peut pas laisser à moitié faite. La base est ouverte avec `secure_delete` : ce qui est supprimé (un coffre, ses versions) est écrasé dans le fichier, et n'apparaît donc pas dans une sauvegarde.

**Sauvegarde du serveur**
- `sudo pswd-server backup <dossier>` : copie cohérente de la base et du certificat (sans arrêter le serveur) dans un nouveau fichier `pswd-server-sauvegarde_<AAAA-MM-JJ_HH-MM-SS>.db` ; le dossier doit exister, rien n'est écrasé.
- Le panneau d'administration de PSWD Master propose « Sauvegarder le serveur » : le serveur produit une copie équivalente (`GET /v1/admin/backup`, base SQLite complète, cohérente car faite sous le verrou de la base) et l'application l'enregistre par la fenêtre système sous le même nom. Réservé aux administrateurs (la copie contient les coffres chiffrés de tous les comptes).
- `sudo pswd-server restore <fichier>` : seulement sur un serveur neuf, c'est-à-dire sans base ou avec une base sans compte ni coffre (celle créée par l'installation), service arrêté. Le fichier doit être une base de `pswd-server` (schéma connu, certificat présent), ouverte en lecture seule pour la vérifier ; il est copié à côté puis renommé, pour ne jamais laisser une base à moitié copiée. Le certificat restauré garde la même empreinte : les appareils se reconnectent sans rien refaire.
- Une commande lancée avec `sudo` (`reset-admin`, `restore`) redonne ensuite la base au propriétaire du dossier des données : le service doit pouvoir l'écrire. La base attend jusqu'à 5 s qu'une autre connexion la libère (`busy_timeout`), pour qu'une sauvegarde passe pendant que le service écrit.
- Pas de sauvegarde automatique : une copie sur le même disque ne protège pas de sa panne.

**Transport** : HTTPS, **TLS 1.3 seulement** (serveur et client). Le serveur génère son propre certificat au premier démarrage (`rcgen`, ECDSA P-256), sans expiration pratique (fin de validité au 31/12/9999), et le range avec sa clé privée dans `pswd.db` : une sauvegarde de la base emporte le certificat. Sur SIGTERM (arrêt par `systemd`) ou Ctrl+C, il laisse 10 s aux requêtes en cours avant de s'arrêter. L'application l'**épingle** : elle ne vérifie que son empreinte SHA-256 (`certificate_fingerprint` de `pswd-core`), ni les dates ni le nom, ce qui permet les deux adresses. Client (`ServerClient` de `pswd-infra`) : attente de connexion de 5 s par adresse, 2 min au plus par requête ; une adresse injoignable ou au mauvais certificat fait passer à la suivante. Si aucune ne répond, « serveur injoignable », ou « le serveur n'est pas celui attendu » si l'une d'elles présentait un autre certificat. Un certificat différent est refusé (« Le serveur n'est pas celui attendu »), sans jamais proposer de lui faire confiance. Pas de changement de certificat pour l'instant (§10 `cert-rotation`) : une clé privée volée permettrait de se faire passer pour le serveur et de récupérer des jetons, jamais de lire un coffre.

**API** : REST sous le préfixe `/v1/`. Le contenu d'un coffre circule en octets bruts (`application/octet-stream`), tout le reste en JSON. Jeton dans l'en-tête `Authorization: Bearer <jeton>`. Version d'un coffre = compteur entier tenu par le serveur, augmenté à chaque publication.

| `RemoteStore` | Requête | Réponses |
|---|---|---|
| — | `POST /v1/vaults` `{name, shared}` | `201` + `{"id": "<uuid>"}` : emplacement réservé, encore vide ; `shared` (faux par défaut) en fait un coffre commun ; `400` nom invalide |
| `fetch()` | `GET /v1/vaults/{id}` | `200` + octets + `ETag: "<version>"` ; `204` jamais publié (`None`) ; `404` inconnu ; `410` + `{"reason": "deleted"}` coffre supprimé par un administrateur, `{"reason": "deleted_everywhere"}` supprimé partout par un membre, ou `{"reason": "left"}` le compte n'en est plus membre |
| `push(…, Some(v))` | `PUT /v1/vaults/{id}` + `If-Match: "<v>"` | `204` + nouvel `ETag` ; `412` le coffre a bougé (`Conflict`) |
| `push(…, None)` | `PUT /v1/vaults/{id}` + `If-None-Match: *` | `204` + `ETag: "1"` ; `412` déjà publié |

Une publication sans condition reçoit `428` ; un `If-Match` illisible ne correspond à aucune version (`412`) ; un fichier qui n'est pas un `vault.enc` valide reçoit `400`, un envoi de plus de 64 Mio `413`. Publier un nouveau coffre se fait en deux temps : l'application réserve l'identifiant (`POST`) et le note avant de publier (`PUT`), pour qu'une coupure entre les deux ne crée jamais de doublon. Le créateur d'un coffre en devient membre ; un coffre dont le compte n'est pas membre répond `404`, comme un coffre inconnu.

Autres routes (JSON, 16 Kio au plus par requête) :

| Requête | Accès | Effet |
|---|---|---|
| `POST /v1/connect` `{key, device_name, account_name?}` | libre | `201 {token, account_id, device_id, is_admin}` ; `401` clé invalide, expirée ou déjà utilisée ; `400` nom invalide ou absent (une clé administrateur crée un compte nommé par `account_name`) |
| `GET /v1/vaults` | appareil | `200 [{id, name, shared, version}]` : coffres du compte, `version` = `ETag` ou `null` |
| `GET /v1/accounts` | appareil | `200 [{id, name}]` : comptes du serveur, triés par nom, pour choisir qui inviter |
| `GET /v1/vaults/{id}/members` | membre | `200 {members: [{id, name}], invitations: [{id, account: {id, name}}]}` : membres et invitations en attente |
| `POST /v1/vaults/{id}/invitations` `{account_id}` | membre | `201 {id}` ; `400` coffre privé ou compte inconnu ; `409` déjà membre ou déjà invité |
| `GET /v1/invitations` | appareil | `200 [{id, vault_id, vault_name, invited_by}]` : invitations reçues par le compte |
| `POST /v1/invitations/{id}/accept` | invité | `204` : le compte devient membre ; ses appareils téléchargent le coffre au prochain échange |
| `DELETE /v1/invitations/{id}` | invité ou membre | `204` : invitation refusée (invité) ou annulée (membre du coffre) |
| `DELETE /v1/vaults/{id}/members/self` | membre | `204` : le compte quitte le coffre commun ; `400` coffre privé |
| `POST /v1/members` `{name}` | administrateur | `201 {key}` : clé d'un nouveau membre ; `403` pour un non-administrateur |
| `POST /v1/devices/keys` | appareil | `201 {key}` : clé d'un appareil de plus pour le même compte |
| `DELETE /v1/devices/self` | appareil | `204` : déconnexion, le jeton est révoqué |
| `GET /v1/server` | appareil | `200 {account_id, account_name, is_admin, public_address}` : repris à chaque échange (rôle, adresse publique) |
| `DELETE /v1/vaults/{id}` | membre | `204` : le coffre est supprimé partout (effet ci-dessous) ; possible pour le propriétaire d'un coffre privé et pour le dernier membre d'un coffre commun ; `400` coffre commun qui a d'autres membres : il se quitte |

Routes d'administration (`403` pour un appareil dont le compte n'est pas administrateur ; dates en millisecondes depuis 1970) :

| Requête | Effet |
|---|---|
| `GET /v1/admin/accounts` | `200 [{id, name, is_admin, devices: [{id, name, created_at}]}]`, triés par nom |
| `DELETE /v1/admin/accounts/{id}` | `204` : compte supprimé avec ses appareils, ses clés et les invitations qu'il a reçues ou envoyées ; ses coffres restent sur le serveur, ceux dont il était le seul membre n'en ont plus ; `404` ; `409` dernier administrateur, ou autre administrateur que l'appelant (il perd d'abord son rôle) |
| `PUT /v1/admin/accounts/{id}/admin` `{is_admin}` | `204` ; `404` ; `409` retirer son rôle au dernier administrateur. Le rôle est relu à chaque requête : le changement vaut aussitôt |
| `DELETE /v1/admin/devices/{id}` | `204` : appareil révoqué ; `404` |
| `GET /v1/admin/keys` | `200 [{id, purpose, name, expires_at}]` : clés de connexion inutilisées et valables ; `purpose` = `admin`, `member` (`name` : compte à créer) ou `device` (`name` : compte de l'appareil) |
| `DELETE /v1/admin/keys/{id}` | `204` : clé annulée ; `404` |
| `GET /v1/admin/vaults` | `200 [{id, name, shared, version, members: [{id, name}]}]` : tous les coffres non supprimés, triés par nom ; `version` entière ou `null` |
| `DELETE /v1/admin/vaults/{id}` | `204` : coffre supprimé (contenu, versions et invitations effacés ; ses membres en gardent la trace et reçoivent `410`) ; `404` |
| `POST /v1/admin/vaults/{id}/members` `{account_id}` | `204` : rattache un compte à un coffre sans membre ; `400` compte inconnu ; `404` ; `409` le coffre a encore des membres |
| `DELETE /v1/admin/vaults/{id}/members/{account}` | `204` : membre retiré d'un coffre commun (il reçoit `410`, raison `left`) ; `400` coffre privé, qui n'a que son propriétaire et se supprime ; `404` |
| `GET /v1/admin/vaults/{id}/versions` | `200 [{version, created_at, size}]`, de la plus récente à la plus ancienne ; `404` |
| `POST /v1/admin/vaults/{id}/versions/{v}/restore` | `201 {id, name}` : nouveau coffre `<nom>-v<v>` (nom raccourci pour tenir en 32 caractères), même type, mêmes membres, publié en version 1 ; les appareils le reçoivent au prochain échange ; `404` |
| `PUT /v1/admin/settings` `{public_address}` | `204` ; `null` efface l'adresse ; `400` adresse invalide (vide, plus de 255 caractères, espace, caractère de contrôle, ou l'un de `/ ? # @ \`) |
| `GET /v1/admin/backup` | `200` + octets : sauvegarde du serveur |

Noms de compte et d'appareil : 1 à 64 caractères, ni espace en bordure, ni caractère de contrôle. Jeton : 32 octets aléatoires en hexadécimal, montré une seule fois.

Le serveur identifie les coffres par son propre identifiant (UUID v4), jamais par leur nom. Il garde aussi ce nom (nom de coffre valide, §7), donné à la réservation : un appareil qui télécharge un coffre ne pourrait pas le nommer autrement (le nom n'est pas dans `vault.enc`), et un invité devra savoir à quel coffre il est invité. Le serveur voit donc les noms des coffres en clair, jamais leur contenu.

**Protections**
- Taille maximale : 64 Mio pour un coffre, quelques Ko pour une requête JSON ; délai maximal de 30 s par requête.
- Au-delà de 10 jetons ou clés invalides par minute depuis une adresse IP, celle-ci est bloquée 5 minutes (`429`). Une requête sans jeton n'est pas comptée.
- Un coffre reçu est vérifié (structure, version, en-tête, par `pswd-core`, sans clé) ; un envoi mal formé est refusé.
- Journal (`journalctl -u pswd-server`) : démarrages, erreurs, accès refusés ; jamais de jeton, de clé ni de contenu.
- Une requête non autorisée reçoit `401` sans détail : le serveur ne révèle pas l'existence d'un coffre à qui n'y a pas droit.

**Accès depuis l'extérieur : prochaine version.** Il n'est pas encore implémenté : en v1.0.0, la synchro se fait sur le réseau de la maison (adresse locale). La conception prévue est la suivante.

**Accès** (prévu) : l'application retient une adresse locale et une adresse publique, essayées dans cet ordre (délai max 5 s). L'adresse publique (nom DNS ou adresse IP, port facultatif) est renseignée par un administrateur dans le panneau ; le serveur ne la devine pas (il est derrière la box). Chaque appareil la reçoit à chaque échange réussi (`GET /v1/server`) et met `server.toml` à jour. Un changement d'adresse est sans risque : le certificat est vérifié quelle que soit l'adresse. Un appareil hors de chez soi qui ne joint plus l'ancienne adresse la reçoit dès qu'il repasse par l'adresse locale : un nom DNS dynamique est conseillé plutôt qu'une adresse IP qui change. Il n'y a pas de relais.

**Administration**
- À l'installation, le serveur affiche une fois son adresse et une **clé administrateur** (clé de connexion, ci-dessous). Le premier appareil qui la présente devient administrateur ; personne d'autre ne le peut.
- Secours : sur la machine, `pswd-server reset-admin` affiche une nouvelle clé administrateur (aussi si la première a expiré) et annule les précédentes. Présentée, elle crée un **nouveau** compte administrateur ; l'ancien compte reste, à supprimer depuis le panneau. Le serveur se lance par `pswd-server serve` (`--config`, par défaut `/etc/pswd-server/config.toml` ; `--listen` remplace l'écoute sur toutes les interfaces, par exemple `127.0.0.1:8443` en développement) ; toutes les sous-commandes (`serve`, `reset-admin`, `backup`, `restore`) prennent `--data-dir` (par défaut `/var/lib/pswd-server`). Les noms datés des sauvegardes (`.pswd` et serveur) viennent de `pswd-core` (`utc.rs`), partagé par l'application et le serveur.
- Le panneau d'administration de PSWD Master permet d'ajouter un membre, de supprimer un compte, de révoquer un appareil, de promouvoir un membre administrateur ou de lui retirer ce rôle, de supprimer un coffre, de rattacher quelqu'un à un coffre sans membre, de restaurer une version, de voir l'adresse publique (champ grisé, `EXTENSION(acces-exterieur)`) et de sauvegarder le serveur. Le dernier administrateur ne peut pas perdre son rôle ni être supprimé, et son dernier appareil ne se révoque pas depuis le panneau (le serveur répond `409` ; décidé le 2026-11-12) : plus personne ne pourrait sinon administrer le serveur. Ses autres appareils se révoquent (un téléphone volé, depuis le PC) ; pour révoquer le dernier, nommer d'abord un autre administrateur. La déconnexion d'un appareil par lui-même reste toujours possible. Un administrateur ne supprime pas le compte d'un autre administrateur (il lui retire d'abord son rôle), mais peut supprimer le sien s'il n'est pas le dernier : l'appareil oublie alors le serveur et garde ses coffres, comme à une déconnexion avec copies (décidé le 2026-11-11, test A2-03). Il voit aussi les clés de connexion en attente (usage, expiration) et peut les annuler. Restaurer une version sert à rattraper une erreur que la fusion a propagée partout (entrée supprimée, mot de passe d'un coffre commun changé puis oublié) : la restauration crée un nouveau coffre, car remplacer le contenu serait aussitôt défait par la fusion avec les copies des appareils.
- Le panneau s'ouvre depuis l'écran « Réglages » (bouton « Administration ») ; le bouton n'apparaît que si le serveur répond que le compte est administrateur. L'écran « Serveur » montre le compte de l'appareil. Quatre onglets : Comptes (appareils, rôle, suppression, ajout d'un membre avec sa clé montrée une fois ; un membre dont la clé n'a pas encore servi n'a pas de compte sur le serveur, il apparaît donc d'après sa clé, marqué « en attente », avec l'expiration et « Annuler la clé » ; décidé le 2026-11-11, A2-01 des tests V1.2), Coffres (membres, rattachement, versions et restauration, suppression), Clés (en attente, annulation), Serveur (adresse publique grisée, « Bientôt disponible », et sauvegarde). Supprimer un compte ou un coffre du serveur se confirme en retapant son nom.
- Retouches du panneau (décidées le 2026-11-12) : « Retour » est une icône en haut à gauche, comme le retour d'une entrée sur mobile ; la sauvegarde du serveur, une icône à droite de son titre. « Ajouter un membre » est un bouton qui ouvre le formulaire (nom, puis « Annuler » et « Enregistrer » côte à côte). Révoquer un appareil et annuler une clé (onglets Comptes et Clés) demandent une confirmation. Les clés en attente sont découpées en pages sur le bureau, comme les comptes. Comptes, coffres et clés sont relus toutes les 5 s (même rythme que l'échange automatique, §7), sans message en cas d'échec, sauf pendant une opération ou une confirmation ; le volet d'un coffre n'est recréé que si ce coffre a changé.
- Nom d'un nouveau membre (décidé le 2026-11-12) : refusé s'il est déjà celui d'un compte ou d'un membre en attente (clé de nouveau membre encore valable), comparé à l'identique comme les noms de coffres (Camille ≠ camille). L'interface le vérifie avant l'envoi, le serveur le refuse aussi (`409`). Une clé administrateur (installation, `reset-admin`) n'est pas concernée : le compte qu'elle crée peut reprendre le nom de l'ancien.
- Le panneau n'affiche que les actions possibles (décidé le 2026-11-09, #13 des tests d'intégration) ; le serveur refuse quand même les autres. Le dernier administrateur porte la mention « Dernier administrateur », sans « Retirer le rôle » ni « Supprimer le compte » (recalculé après chaque changement de rôle). Le compte de l'appareil porte « Supprimer mon compte » (sauf dernier administrateur) ; après la suppression, le panneau se ferme sur la liste des coffres. Un autre administrateur n'a pas « Supprimer le compte ». Le compte de l'appareil ne se retire pas d'un coffre (il le quitte, ci-dessus). Le membre d'un coffre privé, son propriétaire, ne se retire pas. La version actuelle d'un coffre porte « (actuelle) », sans restauration : elle créerait une copie identique. Les boutons d'une ligne montrent « Retirer » ou « Révoquer » ; leur étiquette d'accessibilité nomme le membre ou l'appareil. Pendant une confirmation (suppression d'un compte ou d'un coffre, révocation d'un appareil, annulation d'une clé), le « Retour » du panneau est masqué : « Annuler » suffit. L'identifiant du compte de l'appareil vient de `GET /v1/server` (`AccountStatus.accountId`).
- Mise en page du panneau (décidée le 2026-11-10, #14 des tests d'intégration) : carte plus large que les autres écrans (60 rem), comptes et coffres en cartes sur deux colonnes sur le bureau, contenu des cartes de coffre aligné à gauche. Comptes, coffres et versions d'un coffre sont découpés en pages sur le bureau, comme les entrées (§12, composant `Paged.svelte`, dimensions dans `src/lib/layout.ts`). Comptes et coffres se cherchent par nom, sans tenir compte des accents ni des majuscules ; le filtre est fait par l'interface, car ces listes sont déjà chargées en entier et restent courtes. Une nouvelle recherche revient à la première page.

**Comptes et appareils**
- Ajouter un membre produit l'adresse du serveur et une **clé de connexion**, à recopier dans l'application (ordinateur ou mobile). Même format que la clé de récupération (§5 : Base32 Crockford, 52 caractères en 13 groupes, même règle de saisie), portant 32 octets : 16 octets de secret aléatoire, puis les 16 premiers octets de l'empreinte du certificat. L'application vérifie que le certificat présenté commence par ces octets, puis retient l'empreinte complète : le serveur est vérifié dès la première connexion, sans comparaison manuelle.
- Une clé de connexion est à usage unique, valable 7 jours, annulable depuis le panneau. Le même format sert pour la clé administrateur, l'ajout d'un membre et l'ajout d'un appareil à un compte existant (clé générée depuis un appareil déjà connecté du compte).
- Chaque appareil reçoit son propre jeton, révocable individuellement. Le serveur ne garde que l'empreinte (SHA-256) des jetons et des clés de connexion : une copie de la base ne donne aucun accès. Le compte n'a pas de mot de passe serveur : rien ne se confond avec le mot de passe maître.
- Sur l'appareil, l'adresse locale, l'adresse publique, l'empreinte du certificat et le jeton sont rangés dans un fichier, `<data_dir>/pswd/server.toml` (`local_address`, `public_address` facultative, `fingerprint` et `token` en hexadécimal ; droits `0600` sous Linux ; dossier privé de l'application sur mobile). Son existence signifie qu'un serveur est **enregistré** ; l'appareil n'est dit **connecté** que si son dernier contact avec ce serveur a réussi. Pas de trousseau du système : il ne protègerait pas mieux (le `vault.enc` chiffré, que le jeton permet de télécharger, est dans le même dossier), il manque souvent sous Linux sans bureau et exigerait du code natif sur mobile. Un jeton volé se révoque depuis le panneau.
- Un appareil est connecté à un seul serveur. Il peut s'en déconnecter (son jeton est révoqué) puis se connecter à nouveau. En se connectant, il télécharge la copie chiffrée de tous les coffres publiés dont le compte est membre et qu'il n'a pas déjà ; chacun se synchronise ensuite à son ouverture (§7). Les coffres créés plus tard par un autre appareil arrivent par le même échange, relancé automatiquement (§7) ou à la demande (bouton « Synchroniser »).
- **Le nom du serveur fait foi** (décidé le 2026-11-10, #15 des tests d'intégration) : un nom désigne le même coffre sur tous les appareils. Au début de chaque échange, un coffre de l'appareil qui n'est pas sur ce serveur (jamais publié, lié à un autre serveur, ou dont la republication a été demandée) et dont le nom est celui d'un coffre du serveur **cède son nom** : il est renommé `<nom>-2` (le premier numéro libre ici et sur le serveur, nom raccourci pour tenir en 32 caractères), puis publié sous ce nom. Le coffre du serveur est téléchargé sous son nom. Ce coffre local reste un coffre distinct (restaurer n'est pas fusionner, §6) ; le message le signale : « « banque » renommé « banque-2 » : nom déjà pris sur le serveur. » (aussi après un échange automatique). La synchro d'un coffre ouvert ne le renomme pas (l'interface le désigne par son nom) : elle ne le publie pas et répond `NameTaken` ; l'échange suivant, depuis la liste, s'en charge. Reste un cas rare : deux coffres du serveur de même nom pour un compte (coffres communs de deux personnes). Le second téléchargé prend lui aussi `<nom>-2` ; selon l'ordre d'arrivée, ce n'est pas forcément le même sur chaque appareil (accepté : l'unicité des noms par compte côté serveur a été écartée, trop compliquée). Renommer un coffre (remarque 9 des tests d'intégration) est abandonné (décision du 2026-11-10) : un coffre garde le nom choisi à sa création, ou celui donné par cette règle.
- Le lien d'un coffre avec le serveur est noté dans `config.toml` : `[vaults.<nom>.remote]`, avec `server` (empreinte du certificat, en hexadécimal) et `id` (identifiant sur le serveur). Il est noté dès la réservation, avant la publication.
- Les coffres du serveur retirés de l'appareil (« Retirer de cet appareil », ci-dessous) sont notés dans `config.toml` (`ignored`, liste d'identifiants) : l'échange ne les télécharge plus. Un identifiant que le serveur ne donne plus est oublié au prochain échange ; la liste est vidée à la déconnexion.

**Coffres**
- Un coffre est **privé** ou **commun**, choisi à sa création dans PSWD Master et jamais changé : le mot de passe d'un coffre privé n'a jamais à être transmis. Un coffre commun est protégé par un mot de passe maître partagé, transmis hors du serveur. Le serveur garde ce type (`shared`), donné à la réservation ; un coffre local retient le sien dans `config.toml` (`shared = true`) jusqu'à sa publication.
- Tout membre d'un coffre commun peut inviter un utilisateur du même serveur, choisi dans la liste des comptes, et annuler une invitation en attente. L'invité voit ses invitations en haut de la liste des coffres (demandées au serveur à chaque échange : connexion, « Synchroniser », échange automatique) ; il accepte ou refuse. Accepter le rend membre : le coffre est téléchargé aussitôt, et s'ouvre avec le mot de passe que lui donne celui qui l'a invité. Tout membre peut quitter le coffre. Tous les membres sont égaux ; seul un administrateur retire quelqu'un. Après un départ, changer le mot de passe maître du coffre est conseillé : l'ancien membre le connaît. Le panneau « Membres » se relit toutes les 5 s tant qu'il est affiché (sans signaler un échec) : une invitation acceptée ailleurs y passe d'elle-même dans les membres (décidé le 2026-11-11, test K2-01).
- Un compte qui a quitté un coffre, ou en a été retiré, en garde une trace sur le serveur : ses appareils reçoivent `410` (raison `left`), et non `404`, et suppriment leur copie locale au prochain échange ou à la prochaine synchro (modifications non publiées comprises). Une nouvelle invitation acceptée efface cette trace : le coffre est de nouveau téléchargé.
- Être membre donne le droit de télécharger et de publier le fichier ; le lire exige le mot de passe du coffre.
- Le serveur ne supprime jamais un coffre de lui-même, même sans membre. Depuis l'application, un coffre privé publié propose deux choix (décidé le 2026-11-07, #12 des tests d'intégration ; avant, « local seulement » laissait le coffre se retélécharger, et « local et serveur » laissait des copies orphelines sur les autres appareils) :
  - **« Supprimer partout »** (par défaut) : le coffre est supprimé du serveur et de l'appareil ; les autres appareils des membres reçoivent `410` (raison `deleted_everywhere`) et suppriment leur copie au prochain échange ou à la prochaine synchro, sauf une copie qui a des modifications que le serveur n'a pas (ou dont le dernier état commun est inconnu) : elle est gardée, « absente du compte », et rien n'est perdu. Un coffre déjà supprimé du serveur par un autre appareil est seulement supprimé de l'appareil.
  - **« Retirer de cet appareil »** : le coffre reste sur le serveur et sur les autres appareils ; il est supprimé de l'appareil et n'y est plus téléchargé (`ignored`, ci-dessus). Refusé si la copie locale a des modifications que le serveur n'a pas : la synchroniser d'abord. L'écran « Serveur » liste ces coffres et propose de les télécharger de nouveau (`Vaults::download_again`, qui relance un échange).
- Un coffre commun n'a pas de propriétaire : tous les membres sont égaux et le serveur n'enregistre pas de créateur. Il se quitte ; son **dernier membre** le supprime partout (nom du coffre à retaper), pour ne pas laisser un coffre sans membre que seul un administrateur pourrait nettoyer. Pas de propriétaire : il faudrait une hiérarchie des droits, sa transmission au départ du propriétaire et le cas d'un compte supprimé, pour un besoin que couvrent le dernier membre et les administrateurs.
- Dans PSWD Master, le menu d'un coffre commun publié ne propose pas de suppression sur l'appareil seul (le coffre y serait de nouveau téléchargé) : seulement « Quitter ce coffre ». Le panneau lit la liste des membres à son ouverture ; si le compte en est le seul membre, il devient « Supprimer ce coffre » (suppression partout). Membres illisibles : « Quitter » reste proposé. Le même panneau s'ouvre depuis l'écran de déverrouillage d'un coffre commun publié (lien « Quitter ce coffre ») : un invité qui n'a jamais reçu le mot de passe peut ainsi quitter le coffre.
- Une suppression par un administrateur (panneau, nom du coffre à retaper) garde au contraire les copies locales : ses anciens membres reçoivent `410` (raison `deleted`) et gardent leur copie. Un administrateur n'efface pas les données présentes sur les appareils des autres.
- Toute suppression sur le serveur efface aussitôt son contenu, ses versions et ses invitations ; seule reste une trace (identifiant, « supprimé » par un administrateur ou partout : colonne `removed`, 1 ou 2). Pas de corbeille : les copies locales et les sauvegardes suffisent à rattraper une erreur.
- Le serveur conserve les **15 dernières versions** de chaque coffre. Une restauration crée un nouveau coffre avec les mêmes membres (§6).

**État d'un coffre sur un appareil** (calculé à partir de la liste des coffres du serveur, `Vaults::vault_states` ; aucun état sans serveur enregistré ou si le serveur est injoignable)

Règle des couleurs : **rouge** dès que le coffre n'a pas de lien avec le serveur (jamais publié, lié à un autre serveur, absent du compte sur le serveur), même s'il est gardé sur l'appareil, et sur tous les coffres tant que les échanges avec le serveur enregistré échouent (pastille « Échanges avec le serveur impossibles : synchro en pause ») ; **orange** quand le lien existe mais que les deux copies diffèrent (nuance : rien n'est perdu, la synchro peut les réunir) ; **verte** quand elles sont identiques. Aucune pastille sans serveur enregistré.

| État | Pastille | Signification | Comportement |
|---|---|---|---|
| local | rouge | Pas sur ce serveur : jamais publié, ou lié à un autre serveur | Publié automatiquement, tel quel et sans mot de passe, à chaque échange et à chaque synchro (le lien avec un autre serveur est remplacé) |
| à jour | verte | Même contenu que la copie du serveur | — |
| à mettre à jour | orange | Un seul côté a changé depuis l'état commun | Mis à jour sans mot de passe au prochain échange (§7) |
| à déverrouiller | orange | Modifié des deux côtés, ou état commun inconnu | Fusionné à son ouverture ; l'échange le signale |
| absent du compte | rouge | Supprimé du serveur par un administrateur (`410`, raison `deleted`), supprimé partout alors que la copie locale a des modifications (raison `deleted_everywhere`), ou inconnu du serveur pour ce compte (`404` : coffre d'un autre compte laissé sur l'appareil, par exemple) | Signalé par sa pastille seulement, sans message (décidé le 2026-11-11 : la pastille et le formulaire de déverrouillage suffisent) ; gardé en local, noté `removed = true` dans `[vaults.<nom>.remote]`, jamais republié automatiquement : seulement par le bouton « Synchroniser » du coffre ouvert, comme nouveau coffre |

- Un coffre commun dont le compte n'est plus membre (il l'a quitté depuis un appareil, ou un administrateur l'en a retiré) n'a pas d'état : il est supprimé de chaque appareil du compte (`SyncOutcome::Left`). Quitter un coffre le supprime aussitôt de l'appareil.
- De même, un coffre supprimé partout depuis un autre appareil est supprimé de l'appareil s'il n'y a pas changé depuis le dernier état commun (`Exchange::deleted`, `SyncOutcome::Deleted`) ; ouvert, il est d'abord fermé.
- **Limite acceptée** : un coffre republié sur un nouveau serveur oublie l'ancien ; reconnecté plus tard à l'ancien serveur, l'appareil y publierait un doublon.
- À la déconnexion, l'application demande s'il faut garder une copie locale des coffres (oui par défaut). Sans copie, les données restent sur le serveur : seuls sont supprimés de l'appareil les coffres dont l'appareil n'a rien que le serveur n'ait déjà : le coffre figure encore dans la liste du serveur, et l'empreinte de la copie locale est celle du dernier état commun (`synced.file_hash`). Le serveur a pu avancer depuis : il ne fait qu'ajouter à cet état, il n'y a donc rien à perdre. Les autres (modifications non publiées, jamais synchronisé, absent du serveur, serveur injoignable) sont gardés et signalés.
- Déconnexion : vérification des copies (si demandée), révocation du jeton, suppression de `server.toml`, puis des copies locales. Un serveur injoignable n'empêche pas de se déconnecter : le jeton, non révoqué, est signalé, à révoquer depuis le panneau.
- Pour l'application, « coffre inconnu » (`404`) et « coffre retiré » (`410`, raison `deleted`) sont un même état : le coffre n'est pas sur le compte, et le serveur ne dit pas s'il existe (§7). `RemoteStore` reçoit dans les deux cas une erreur de retrait (`PortError::removed`) ; l'adaptateur HTTP retient la raison (`RemoteVault::removal` : `Left`, `Deleted`, `DeletedEverywhere`, ou `Absent` pour un `404`) et la dernière version vue (`RemoteVault::version`). Seuls `Left`, et `DeletedEverywhere` pour une copie inchangée, suppriment la copie locale. Décidé le 2026-11-07 (#11 des tests d'intégration) : avant, un coffre d'un autre compte resté sur l'appareil répétait l'erreur « coffre inconnu du serveur » à chaque échange.
- La déconnexion se confirme en une phrase qui compte les coffres retirés et gardés (« Appareil déconnecté : 3 coffres retirés, 1 gardé. ») ; seul un coffre gardé à cause d'une erreur est nommé. Le rappel sur les coffres d'un autre compte est abandonné (2026-11-11) : après une nouvelle connexion, leur pastille « absent de votre compte » les signale.

## 12. Interfaces graphiques (étape 2)

**Application** : PSWD Master, identifiant `fr.arthurfontaine.pswdmaster` (modifiable jusqu'à une première publication sur un store). Un seul projet Tauri 2 pour Windows, Linux, macOS, Android et iOS ; une seule branche `dev-interface`. Les différences entre plateformes se règlent dans le code (`#[cfg(target_os = …)]`), jamais par des branches.

**Organisation** : `crates/pswd-gui/` contient l'interface (`package.json`, `src/`, Vite) et `crates/pswd-gui/src-tauri/` la crate Rust `pswd-gui` (membre du workspace), avec `tauri.conf.json`, `capabilities/` et le projet Android généré (`gen/android`). Les commandes appelables sont déclarées dans `build.rs` et autorisées une à une dans `capabilities/default.json` ; un test vérifie qu'une commande existante mais non autorisée est refusée.

**Fonctionnement** : la webview du système affiche l'interface (HTML/CSS/JS intégrés à l'exécutable, aucun serveur web, aucun réseau). L'interface appelle les commandes Tauri du backend Rust, qui s'appuie sur `pswd-app`. La session (clé du coffre, coffre déchiffré) reste en Rust. Règles de sécurité : §9.

**Interface** : Svelte 5 + TypeScript, construite par Vite ; CSS maison avec des variables (couleurs, espacements), sans bibliothèque de composants. Police Atkinson Hyperlegible Next (paquet `@fontsource-variable`, embarquée : aucun réseau), choisie pour sa lisibilité (l, I et 1, O et 0 distincts).
- Thème clair ou sombre selon le système, réglable (auto, clair, sombre). C'est le seul réglage de l'écran « Réglages » : les délais (verrouillage, presse-papiers, arrière-plan) gardent leurs valeurs par défaut, modifiables à la main dans `config.toml` sur le bureau.
- Mise en page de départ, appelée à évoluer : deux colonnes sur le bureau (liste à gauche, détail à droite), une colonne sur mobile (liste, puis détail en plein écran). Sur un très grand écran (à partir de 1600 × 896 px), la taille de base passe à 18 px, puis 20 px (à partir de 2240 × 1200 px) : tout grandit ; le contenu s'élargit et, hors coffre ouvert, descend vers le centre. Sur l'écran d'accueil, les cartes des coffres ont toutes la même taille : un nom trop long est coupé par « … » (le nom complet reste au survol et à l'ouverture).
- Tout champ de mot de passe (entrée, mot de passe maître) a une icône en forme d'œil qui l'affiche en clair ou le masque de nouveau.
- Le numéro de version (`v1.0.0`) s'affiche en petit à droite du titre ; il vient de `tauri.conf.json`, lu par Vite à la construction (`__APP_VERSION__`). La version est aussi dans `Cargo.toml` et `package.json` : les trois se changent ensemble.
- Logo (décidé le 2026-11-12) : le bouclier coché de l'interface, blanc sur le dégradé d'accent (`#5b5bd6` → `#9d4edd`), le même sur toutes les plateformes. Sources SVG dans `src-tauri/app-icon/` ; toutes les icônes (bureau, Android dans `gen/android`, iOS) en sont tirées par `npx tauri icon src-tauri/app-icon/icon.json`, à relancer après un changement du logo ou un `tauri ios init`.
- Les confirmations (entrée enregistrée, copie, import, sauvegarde, restauration, suppression…) s'affichent dans une notification passagère (`src/lib/toast.svelte.ts`) ; les erreurs restent dans l'écran concerné. Une notification tient en quelques mots, et n'apparaît pas si l'écran montre déjà l'information (pastille, liste, bandeau) : décidé le 2026-11-11 (tests V1.2). Sa couleur suit celle des pastilles : verte pour une confirmation ; le texte d'une pastille touchée s'affiche dans la couleur de la pastille.
- Bureau entièrement utilisable au clavier (Tab, Entrée, Échap, `Ctrl+F`) ; contrastes suffisants.
- Tactile : cibles d'au moins 48 px sur un écran tactile (`pointer: coarse`), survol réservé aux pointeurs qui survolent, marges des zones sûres (affichage bord à bord d'Android 15+, encoche) ; les champs saisis tels quels (nom de coffre, identifiant, clé de récupération, adresses, tags, recherche) désactivent majuscule et correction automatiques (`src/lib/keyboard.ts`).
- Bouton retour d'Android (`src/lib/back.ts`) : chaque écran ouvert s'inscrit, le dernier répond en premier. Il referme le formulaire ou le panneau, puis l'entrée ; dans la liste des entrées, il verrouille le coffre ; ailleurs, il revient à la liste des coffres, d'où il ferme l'application. La clé de récupération ne se quitte que par « Continuer ». Permissions dans `capabilities/mobile.json`, limitée à Android (écoute du bouton, fermeture) ; un test vérifie qu'elle ne s'applique pas au bureau.
- Français d'abord ; tous les textes passent par `src/lib/i18n/<langue>.ts` (cf. `EXTENSION(anglais)`), sauf l'aide ci-dessous.
- **Barre de la liste des coffres** (décidée le 2026-11-13) : sur un écran large (`WIDE_QUERY`, 48 rem), « Nouveau coffre », « Serveur », « Réglages » et « Aide » en icônes ; sur un écran étroit, « Nouveau coffre » reste seul en icône et les trois autres passent dans un menu (`AppMenu.svelte`) : bouton à trois barres qui se croisent en « × » à l'ouverture, menu déployé depuis le bouton (zoom et fondu), refermé par un choix, son bouton, Échap, un clic ailleurs ou le bouton retour d'Android.
- **Aide** (décidée le 2026-11-13) : une icône « ? » dans la barre de la liste des coffres ouvre l'écran « Aide », qui affiche le `README.md` du dépôt, puis le guide d'utilisation (`docs/GUIDE.md`) par son lien. Les deux documents s'adressent à l'utilisateur (rien de technique, aucun lien vers un document absent de l'application) ; la documentation technique reste dans `docs/` (`SPEC.md`, `INSTALLATION.md`, `SERVEUR.md`). Ils sont lus par Vite à la construction (`vite.config.ts`, constantes `__HELP_README__` et `__HELP_GUIDE__`) et convertis par `marked` (`src/lib/help.ts`) : disponibles hors ligne, identiques sur toutes les plateformes, toujours ceux de la version installée. Les titres reçoivent l'ancre calculée comme GitHub, pour que le sommaire marche aux deux endroits ; un test vérifie que chaque lien des deux documents mène quelque part. Liens : ancre → défilement ; `README.md` / `GUIDE.md` → changement de page ; adresse web → navigateur du système (`open_url`) ; tout autre lien est ignoré. « Retour » (et le bouton retour d'Android) ramène du guide au README, puis à la liste. La carte prend toute la largeur de l'application (64 rem, 80 rem sur un très grand écran). Pas d'image : la CSP n'a pas à changer.

**Fichiers et dossiers**
- Bureau : les dossiers standard du système (`Locations::system`, §7), sauf pour la version de développement (`npm run tauri dev`, compilée en debug) qui a les siens, `pswd-dev` à la place de `pswd` (`Locations::system_dev`, décidé le 2026-11-15) : elle ne touche jamais les coffres, la configuration ni le serveur de la version installée. Deux instances de l'application peuvent modifier le même coffre à la fois : chaque écriture fusionne d'abord la copie sur le disque (§3, Session). Une instance ne voit les modifications de l'autre qu'à sa prochaine écriture ou réouverture. **Limite acceptée** : sans verrou de fichier, deux écritures dans la même fraction de seconde peuvent encore se recouvrir.
- Mobile : le dossier privé de l'application fourni par Tauri (`app_data_dir`), passé à `Locations::new` ; `FileStore` fonctionne tel quel.
- Sauvegarde : un fichier `.pswd` (§7), sur le bureau comme sur mobile. « Sauvegarder » liste les coffres de l'appareil, chacun avec une case à cocher (et « Tout sélectionner », aucune cochée au départ) ; la confirmation ouvre la fenêtre système d'enregistrement et écrit les coffres cochés dans un seul fichier. « Restaurer » ouvre le sélecteur de fichiers et liste les coffres du fichier, chacun avec une case à cocher (et « Tout sélectionner ») ; un bouton de confirmation restaure les coffres cochés un par un selon les règles du §7. Un fichier refusé affiche seulement « Sauvegarde inutilisable. », sans le détail technique (#17 des tests d'intégration). Si l'un échoue, ceux déjà restaurés le restent et quittent la liste. `pswd-app` restaure à partir des octets du fichier, pas d'un chemin. Les fichiers choisis sont lus et écrits par Rust (module `files`), jamais par l'interface ; une écriture se termine par `sync_all`.
- Les fenêtres système du mobile renvoient une adresse (`content://` sur Android), pas un chemin : Rust ouvre les fichiers choisis par le plugin `fs` (`FsExt::open`), qui accepte les deux. L'interface n'a aucune permission de ce plugin (vérifié par un test). L'interface n'affiche jamais un chemin complet : la sauvegarde donne seulement le nombre de coffres, la restauration le nom du fichier `.pswd` quand l'adresse le contient (rien sinon). **À vérifier sur le Mac** : sur iOS, le plugin ouvre un accès « security-scoped » à chaque fichier, qu'il faudrait refermer.

**Consultation** : la liste et le détail ne contiennent jamais de mot de passe. « Copier » est fait par Rust (`copy_password`) ; « Afficher » (`reveal_password`) envoie le mot de passe à l'interface, qui le masque de nouveau au bout de 30 s, ou dès qu'on change d'entrée. Recherche à chaque frappe (côté Rust, `pswd_app::search`), filtre par tag, `Ctrl+F` pour aller à la recherche. Sur le bureau, la liste est découpée en pages qui tiennent dans la fenêtre sans défiler (`src/lib/layout.ts`) ; une nouvelle recherche revient à la première page. Le menu « Gérer le coffre » donne accès au changement de mot de passe maître, à l'import CSV et à la suppression du coffre. Toute lecture du coffre repousse son verrouillage ; si le coffre s'est verrouillé entre-temps, l'interface revient à la liste.

**Modification** : un formulaire unique pour ajouter et modifier. Une entrée est un « Mot de passe » (`login`) ou une « Note sécurisée » (`note`) ; une note n'a ni identifiant, ni mot de passe, ni URL ; URLs une par ligne). Les tags s'affichent en étiquettes, chacune avec une croix pour la retirer (décidé le 2026-11-10, #16 des tests d'intégration) : Entrée, un espace, une virgule ou le bouton « Ajouter » ajoutent le tag saisi (le texte est examiné, car un clavier de téléphone n'envoie pas toujours la touche), et les tags existants du coffre qui contiennent la saisie sont proposés (8 au plus, `lib/tags.ts`). Un tag saisi mais pas encore ajouté est enregistré avec les autres. L'interface montre les tags comme ils seront enregistrés (minuscules, sans doublon) ; Rust les normalise de toute façon (`pswd_core::Tags`). Pour le mot de passe : « Saisir » (le champ est vidé dès l'envoi), « Générer » (longueur 8 à 24, symboles ou non : **tiré en Rust à l'enregistrement**, il ne passe jamais par l'interface), et en modification « Inchangé » (par défaut). Un « Mot de passe » exige un mot de passe, une « Note sécurisée » un texte de note : vérifié par le formulaire et par Rust (codes `emptyPassword`, `emptyNote`) ; l'option « Supprimer » du mot de passe a disparu (décidé le 2026-11-11). Les zones de texte (adresses, notes) ont une hauteur fixe, défilent si besoin et ne se redimensionnent pas ; les champs ne dépassent jamais de leur carte sur mobile. La suppression d'une entrée demande une confirmation dans l'écran. L'historique liste les versions précédentes (date, titre, identifiant, « mot de passe différent »), jamais les anciens mots de passe.

**Écran masqué** (§9) : sur Android, `FLAG_SECURE` est posé dans `MainActivity.kt` (`gen/android`, versionné ; à reporter si le projet Android est régénéré) : écran noir dans les applications récentes, captures et enregistrements refusés. Sur iOS, une vue neutre recouvrira l'interface dès que l'application quitte le premier plan ; elle sera ajoutée au projet Xcode généré sur le Mac. Vérifié à la main sur appareil.

**Presse-papiers** : sur le bureau, `LazyClipboard` de `pswd-infra` (ouvert à la première copie). Sur mobile, un plugin maison implémente `SecureClipboard` avec les marqueurs du §9 ; le plugin officiel ne les pose pas.
- Android : classe `SecureClipboardPlugin.kt` dans le projet Android de l'application (`gen/android`), enregistrée par `android_clipboard.rs` (plugin Tauri déclaré dans la crate, sans crate séparée). Marqueur `android.content.extra.IS_SENSITIVE`. Chaque copie porte un identifiant aléatoire dans les extras de sa description, pour vérifier avant d'effacer que le presse-papiers la contient encore ; seul le minuteur de la dernière copie efface.
- **Limites Android** : depuis Android 10, seule l'application qui a le focus peut lire le presse-papiers. Si le délai expire pendant qu'on colle dans une autre application, l'effacement se fait sans vérifier : une copie faite ailleurs entre-temps est effacée aussi. Si le système tue l'application avant le délai, le minuteur disparaît : l'identifiant et l'échéance, gardés dans les préférences de l'application (rien de dérivé du mot de passe), font effacer la copie au lancement suivant. Le mot de passe traverse JNI dans une chaîne JSON, non effaçable, comme toute chaîne Java. L'historique du presse-papiers proposé par le clavier (Gboard, clavier Samsung…) appartient au clavier, pas à Android : aucune application ne peut l'effacer. Les claviers récents respectent `IS_SENSITIVE` et n'y gardent pas la copie ; les autres l'y gardent (test M-06). C'est une limite connue, sans correctif possible côté application : désactiver l'historique du clavier reste à la charge de l'utilisateur.
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

**Serveur** (étape 3, §11)
- Écran « Serveur », ouvert par un bouton de la barre du haut. Aucun serveur enregistré : formulaire de connexion (adresse, clé de connexion, nom de l'appareil, nom du compte, ce dernier seulement avec la clé administrateur). La clé est effacée du champ dès l'envoi ; une fois connecté, l'appareil échange aussitôt ses coffres avec le serveur, et un échange incomplet est signalé. Serveur enregistré : « Connecté au serveur X. » si le dernier contact a réussi ; sinon « Serveur enregistré : X. » suivi de « Échanges impossibles depuis HH:MM : <raison>. » (heure du premier échec de la série, retenue en mémoire par le backend et effacée au premier contact réussi ; le compte n'est alors pas demandé). Dans les deux cas : « Synchroniser » (échange sans mot de passe, §7), « Ajouter un appareil » (clé de connexion montrée une seule fois, à la demande de l'utilisateur, puis oubliée) et « Se déconnecter » (case « Garder une copie des coffres sur cet appareil », cochée par défaut). Serveur injoignable : « Synchroniser » et « Ajouter un appareil » sont grisés, seuls « Retour » et « Se déconnecter » restent ; l'écran relance l'échange automatique toutes les 5 s, comme la liste, et les débloque dès que le serveur répond.
- Liste des coffres, si un serveur est enregistré : pastille d'état sur la carte du coffre (§11) et bouton « Synchroniser » (échange sans mot de passe). Échange automatique et muet (§7) : au montage de la liste, au retour au premier plan (`visibilitychange`, et `focus` de la fenêtre sur ordinateur) et toutes les 5 s tant que la liste est affichée et la page visible ; le minuteur s'arrête quand la liste disparaît. Une seule commande, `refresh_server`, fait l'échange et renvoie l'état des coffres (`null` si un échange est déjà en cours) ; un serveur qui n'a pas répondu n'est pas recontacté pour lire cet état, ni pour les invitations. Les cartes ne sont redessinées que si les noms des coffres ont changé. Les erreurs ne sont pas affichées (l'écran Serveur indique depuis quand les échanges échouent) ; le bouton « Synchroniser » les montre. Pendant l'échange automatique, un coffre apparu dans la liste donne une notification (« Nouveau coffre « x » reçu. ») ; une invitation est seulement listée en haut.
- Compte rendu d'un échange (« Synchroniser », connexion, invitation acceptée) : un seul coffre échangé est nommé (« Coffre « perso » publié. ») ; plusieurs sont comptés en une phrase (« 2 coffres téléchargés, 1 mis à jour depuis le serveur. »). Restent un par un, parce qu'ils demandent l'attention : coffre renommé au téléchargement, supprimé depuis un autre appareil, quitté, et chaque problème (affiché dans un bandeau qui reste, pas dans la notification). La pastille est un bouton distinct de la carte : son texte s'affiche au survol (ordinateur) et, quand on la touche ou la clique, dans une notification, sans ouvrir le coffre. Le formulaire de déverrouillage reprend la pastille du coffre avec son texte (mobile et ordinateur). Un coffre supprimé du serveur l'explique en plus dans ce formulaire, puis une fois ouvert.
- Coffre ouvert, si un serveur est enregistré : bouton « Synchroniser » (synchro avec fusion ; republie un coffre supprimé du serveur). Vérification toutes les 5 s tant que la page est visible (§7), par la commande `check_vault` (muette, sans résultat) ; une synchro qu'elle déclenche se signale comme les autres synchros automatiques. Un coffre dont le compte n'est plus membre est fermé et supprimé. Les synchros automatiques se font en Rust, dans un autre fil ; à la fin de chacune, Rust envoie l'événement `vault-synced`. Un problème (hors ligne, conflit, copie du serveur refusée, erreur locale) reste affiché dans un bandeau jusqu'à la synchro réussie suivante ; une synchro qui apporte des modifications fait relire la liste.
- Les commandes qui parlent au serveur s'exécutent hors du fil de l'interface (commandes `async`, `spawn_blocking`). La suppression d'un coffre aussi : elle attend la fin d'une synchro en cours, qui pourrait réécrire le coffre.
- Les messages d'erreur du serveur viennent de Rust, en français, comme détail d'un code (`serverError`) ou d'un résultat par coffre (cf. `EXTENSION(anglais)`). Exception : un serveur injoignable a son propre code, `serverUnreachable`, sans détail ; le formulaire de connexion y ajoute « Vérifiez l'adresse saisie. » (ailleurs, l'adresse est déjà enregistrée et la cause est autre).

**Tests**
- `pswd-app` et le backend de `pswd-gui` : 100 % lignes et branches visées, comme `pswd-infra` ; exclusions justifiées (démarrage de Tauri, appel au plugin natif). Les commandes sont testées avec le moteur factice de Tauri (`tauri::test`) et la vraie configuration, capacités comprises.
- Sous Windows, le manifeste de l'application (contrôles communs v6) est lié à tous les exécutables par `build.rs`, sinon les tests ne démarrent pas.
- Kotlin et Swift : réduits au minimum, testés à la main sur un appareil réel.
- TypeScript : Vitest, 100 % des lignes pour la logique (`.ts`) ; composants Svelte testés par comportement avec Testing Library, sans seuil.
- `svelte-check` et ESLint strict sans avertissement ; `package-lock.json` versionné et `npm audit` sans faille, comme `cargo deny`. `npm audit` est toujours lancé en entier (sans `--audit-level`) ; une faille n'est acceptée que si aucun correctif n'existe et qu'elle n'est pas exploitable, et elle est listée ici :
  - `deepmerge` 4.3.1 (GHSA-gcqx-w48m-w2h4, gravité faible), par `@sveltejs/vite-plugin-svelte` : empoisonnement de prototype de l'objet fusionné. Seuls nos fichiers de configuration y passent, à la construction ; la bibliothèque n'est pas dans l'application livrée. Accepté le 2026-10-28, à revoir à chaque mise à jour du plugin.
  - `minimatch` 10.2.6 (GHSA-g3hv-75wg-m249, gravité modérée), par ESLint et `typescript-eslint` : déni de service par un motif de fichiers piégé. Seuls les motifs de notre configuration ESLint y passent, à l'analyse du code ; la bibliothèque n'est pas dans l'application livrée. Le seul correctif proposé (`npm audit fix --force`) rétrograde ESLint d'une version majeure. Accepté le 2026-11-07, à revoir à chaque mise à jour d'ESLint.
- Pas de test de bout en bout (`tauri-driver` : Windows et Linux seulement) : l'application construite est essayée à la main.
- Builds mobiles lancés à la main : Android à la fin de chaque sous-étape qui touche au mobile ; iOS sur le Mac, quand il sera disponible.

**Installateurs** (`npm run tauri build`, dans `crates/pswd-gui`, sur chaque système) : NSIS sous Windows (installation pour l'utilisateur courant, sans droits d'administrateur, en français ; WebView2 téléchargé si absent), `.deb` et AppImage sous Linux, `.app` et `.dmg` sous macOS. Pas de MSI : l'installateur NSIS suffit et n'exige pas WiX. Les installateurs ne sont pas signés (avertissement SmartScreen ou Gatekeeper à la première ouverture) ; pas de mise à jour automatique, qui demanderait un serveur de publication. Android (APK signé) : construit et testé à la main sur un téléphone ; iOS : sur le Mac, plus tard. L'APK de release (`npm run tauri android build -- --apk`) est signé par Gradle (`app/build.gradle.kts`) avec la clé décrite dans `gen/android/keystore.properties`, ignoré par git, la clé elle-même restant hors du dépôt (`docs/INSTALLATION.md`) ; sans ce fichier, l'APK sort non signé.
