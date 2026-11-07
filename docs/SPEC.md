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

**Étape 2 — interfaces graphiques** (terminée sur le bureau et Android ; iOS reporté, en attente d'un Mac, §12) : application **PSWD Master** pour le bureau (Windows, Linux, macOS) et le mobile (Android, iOS), un seul projet Tauri 2. Chaque appareil est autonome et entièrement local, avec les fonctionnalités de la CLI. Sous-étapes, chacune validée et commitée :
1. `pswd-app` : cas d'usage extraits de la CLI, comportement inchangé.
2. Squelette Tauri (`pswd-gui`) : CSP et capacités, premier build Android et iOS ; liste des coffres, création, déverrouillage, verrouillage.
3. Consultation : liste, recherche, tags, détail, copier, afficher. Reportés : ouvrir une URL dans le navigateur du système (plugin et capacité dédiés), copier l'identifiant.
4. Modification : ajout, édition, suppression, historique, générateur.
5. Gestion des coffres : mot de passe maître, récupération, import CSV, sauvegarde et restauration, suppression, réglages.
6. Mobile : presse-papiers sensible, verrouillage en arrière-plan, écran masqué, fichiers `.pswd`, ergonomie tactile.
7. Installateurs, documentation, mutants.

Reste à faire sur le Mac : build iOS, plugin Swift du presse-papiers, écran neutre, accès « security-scoped » (§12).

**Étape 3 — serveur maison** (§11) : `pswd-server`, comptes, coffres privés et communs, synchro entre appareils. Sous-étapes par fonctionnalité, du serveur jusqu'à l'écran, chacune validée et commitée :
1. Serveur minimal : crate `pswd-server`, base SQLite créée au démarrage, lecture et publication conditionnelle d'un coffre, 15 versions conservées ; HTTP simple, sans comptes, réservé aux tests.
2. TLS et client : certificat, `rustls`, adaptateur `RemoteStore` HTTP dans `pswd-infra` avec vérification de l'empreinte, testé contre un `pswd-server` local.
3. Comptes et appareils : jetons, clés de connexion, administrateur initial, `reset-admin`, révocation, membres de chaque coffre.
4. Synchro dans la CLI : `connect`, `disconnect`, `add-device`, `status`, `sync` (commande et shell), états des coffres, téléchargement initial. Le modèle des états a été simplifié le 2026-10-31 (§11) : publication automatique de tout coffre local, synchro sans mot de passe quand un seul côté a changé.
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
| `pswd-cli` | Binaire `pswd` : questions et affichage dans le terminal, shell interactif, verrouillage automatique | `#![forbid(unsafe_code)]` |
| `pswd-gui` | Application Tauri 2 **PSWD Master** : backend Rust (commandes, session, verrouillage) et interface Svelte 5 + TypeScript (§12) | `#![forbid(unsafe_code)]` |

Étape 3 : `pswd-server` (exécutable du serveur, `#![forbid(unsafe_code)]`, §11) et l'adaptateur `RemoteStore` HTTP dans `pswd-infra`. Dépendances : serveur `axum` + `tokio`, `axum-server` (HTTPS), `rustls` (moteur `ring`, ni OpenSSL ni `aws-lc-rs`), `rcgen` (certificat), `rusqlite` avec SQLite intégré (`bundled`) ; client `reqwest` en mode bloquant (synchrone comme le port), avec une configuration `rustls` fournie toute prête et un vérificateur d'empreinte maison. `ureq` a été écarté : sa configuration TLS n'accepte pas de vérificateur personnalisé. Pour les tests : `tower`, `http-body-util`, `futures-util`. Licences ajoutées : ISC (`ring`, `rustls-webpki`, `untrusted`), et CDLA-Permissive-2.0 pour la seule crate `webpki-root-certs`.

**Ports (définis dans `pswd-core`)**
- `LocalStore` : `load() -> Option<bytes>` (`None` : pas de coffre local), `save_atomic(bytes)` (fichier temporaire + renommage).
- `RemoteStore` (étape 3) : `fetch() -> Option<(bytes, version)>` (`None` : distant vide ; une erreur vaut « hors ligne », l'adaptateur borne lui-même l'attente), `push(bytes, expected_version)` (`Conflict` si le distant a bougé ; `expected = None` exige un distant vide).
- `SecureClipboard` : `copy_secret(value, ttl)`.
- Les erreurs des adaptateurs sont des `PortError` (un message, jamais de secret).

**Session** (`pswd-core`) : coffre ouvert, déverrouillé par mot de passe ou clé de récupération (`Credentials`, extensible au fichier-clé). Toute modification est enregistrée localement et marque le coffre comme ayant des modifications non publiées. Avant chaque écriture (modification, changement de mot de passe, synchro), la copie locale est relue : si un autre programme l'a réécrite depuis la dernière lecture ou écriture de la session, elle est fusionnée (§6) ; sinon elle ne l'est pas, car refusionner sa propre écriture, plus ancienne, pourrait annuler à date égale la dernière modification. Un mot de passe changé ailleurs est remplacé par le nouveau. Une copie locale disparue ou inutilisable n'est jamais remplacée : l'écriture est refusée. La synchro (§7) est une opération distincte qui reçoit le distant en paramètre : une session n'a besoin d'aucun distant. Les magasins en mémoire la rendent testable ; PSWD Master l'utilise telle quelle.

La crypto n'est **pas** un port : déterministe, sans E/S, identique partout.

**Dépendances** : `argon2`, `chacha20poly1305`, `hkdf` + `sha2`, `getrandom`, `rand` (générateur de mots de passe), `zeroize`, `serde` + `serde_json`, `csv`, `uuid`, `etcetera` (dossiers standard, licence MIT/Apache, contrairement à `directories` qui tire du MPL-2.0), `toml`, `clap`, `rpassword` (saisie masquée), `shlex` (découpage des commandes du shell), `arboard` (presse-papiers du bureau), `tauri` 2 avec les plugins `dialog` (fenêtres système) et `fs` (fichiers choisis sur mobile). Tests : `proptest`, `tempfile`. Côté interface (npm) : `svelte` 5, `vite`, `typescript`, `@tauri-apps/api` et `plugin-dialog`, la police `@fontsource-variable/atkinson-hyperlegible-next` ; pour les tests et contrôles, `vitest`, Testing Library, `svelte-check`, ESLint ; aucune bibliothèque de composants graphiques.

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
- Clé de récupération : 32 octets aléatoires, affichés **une seule fois** à `init`, encodés en Base32 Crockford par groupes de 4 caractères : 52 caractères en 13 groupes séparés par `-`. À la saisie, seule l'écriture affichée est acceptée, tirets compris, en majuscules ou en minuscules ; les espaces autour (copier-coller) sont ignorés. Aucune autre tolérance : ni espaces à la place des tirets, ni `O` lu `0`. Les 4 bits de complément doivent être nuls (une seule écriture par clé). Deux messages : « mal saisie » si la forme est fausse, « incorrecte » si elle est bonne mais que la clé n'ouvre pas le coffre (ou ne peut être aucune clé).
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
- Config : `<config_dir>/pswd/config.toml`, contenant les coffres nommés (`[vaults.<nom>]` : chemin local), `auto_lock_minutes = 10`, `clipboard_clear_seconds = 20`, `background_lock_seconds = 60` (mobile, §12) et `theme = "auto"` (`auto`, `light` ou `dark`, réglé par PSWD Master).
- Données : un dossier par coffre, `<data_dir>/pswd/vaults/<nom>/`, contenant `vault.enc` (`FileStore`). Plusieurs coffres coexistent ; aucun ne remplace un autre.
- Nom d'un coffre : lettres ASCII, chiffres, `-` et `_`, 64 caractères au plus, hors noms réservés de Windows (`con`, `nul`…) ; il sert de nom de dossier. Un nom pris est refusé tant que le coffre existe. Il n'y a pas de coffre par défaut : chaque commande nomme son coffre.
- Les restes de la v1 (dossiers `repo.git/`, jetons `pswd` du trousseau) ne sont plus utilisés et peuvent être supprimés à la main ; les entrées `url` et `default_vault` de `config.toml` doivent être retirées.

**Sauvegarde** : un seul format sur tous les appareils, un fichier `.pswd` (sur mobile, une sauvegarde ne peut pas être un dossier). Il sert aussi à passer ses coffres d'un appareil à l'autre.
- Contenu : les coffres locaux choisis (dans la CLI : tous, ou un seul avec `--vault` ; dans l'interface : ceux cochés), copiés bruts, toujours chiffrés : aucun mot de passe demandé, rien de lisible sur le disque. Nom proposé : `pswd-sauvegarde_<AAAA-MM-JJ_HH-MM-SS>.pswd` (UTC). Les noms des coffres sont rangés dans le fichier : un sélecteur de fichiers mobile ne donne pas toujours le nom du fichier, et celui-ci a pu être renommé.
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
- **Sans mot de passe** : chaque coffre lié au serveur retient son dernier état commun avec lui (`[vaults.<nom>.remote.synced]` : `version` du serveur et `file_hash`, empreinte SHA-256 de la copie locale à ce moment). Un échange (`pswd sync`, bouton « Synchroniser » de la liste, échange automatique de PSWD Master ci-dessous) compare la copie locale à cette empreinte et la version du serveur à celle retenue : si seule la copie locale a changé, elle est envoyée telle quelle (`If-Match` sur la version retenue) ; si seul le serveur a bougé, sa copie, vérifiée par `VaultFile::check_format`, remplace la copie locale. C'est sans perte : chaque version du serveur et chaque écriture locale contiennent déjà la précédente. Si les deux ont changé, ou si l'état commun est inconnu (copies différentes, coffre lié avant cette règle), il faut déverrouiller le coffre pour fusionner. Une synchro avec clé réussie retient l'état commun (version vue par l'adaptateur, copie locale écrite par la session, `Session::saved_file`).
- La synchro avec fusion exige la clé du coffre, donc un coffre déverrouillé. Moments : au déverrouillage (création et récupération comprises), après chaque modification, à la fermeture s'il reste des modifications non publiées et à la demande (bouton « Synchroniser », commande `sync` de la CLI). Pas de synchro au verrouillage par inactivité : la clé est déjà effacée. La modification est d'abord enregistrée localement ; la synchro se fait ensuite en arrière-plan, sans jamais figer l'interface.
- **Échange automatique** (PSWD Master seulement, la CLI n'a que `sync`) : l'échange sans mot de passe se lance seul tant que la liste des coffres est affichée et l'application au premier plan :
  - au démarrage et à chaque retour sur la liste ;
  - quand la fenêtre ou l'application revient au premier plan ;
  - toutes les 30 s.

  Il ne se lance pas si un échange ou une synchro est déjà en cours (pas de file d'attente), ni en arrière-plan (batterie). Il est muet : la liste et les pastilles se mettent à jour, sans message ; le bouton « Synchroniser » garde le compte rendu détaillé. Coût : sans changement, deux petites requêtes (liste des coffres, informations du serveur) ; un coffre n'est téléchargé que s'il a changé. Plusieurs appareils et comptes à 30 s ne pèsent rien pour le serveur. Conséquence : l'état « à mettre à jour » (§11) est corrigé seul en quelques secondes ; une pastille orange qui reste signale une action nécessaire.
- **Coffre ouvert** (PSWD Master) : toutes les 30 s, tant que l'application est au premier plan, la version du serveur (liste des coffres) est comparée à celle du dernier état commun, sans toucher à la session ; si elle a changé, synchro avec fusion. Cette vérification et cette synchro ne comptent pas comme une activité : elles ne repoussent pas le verrouillage par inactivité. Rien pour un coffre supprimé du serveur ou absent de la liste.
- **Mot de passe refusé au déverrouillage** (PSWD Master) : si un serveur est enregistré et le coffre lié, la copie locale est mise à jour depuis le serveur, sans mot de passe (§7, seulement si elle n'a pas changé depuis l'état commun), puis le déverrouillage est réessayé **une fois** avec la même saisie. Un mot de passe changé sur un autre appareil fonctionne ainsi aussitôt. Si rien n'a changé sur le serveur, ou s'il est injoignable (attente de 5 s au plus), le refus est affiché tel quel. Avec l'ancien mot de passe sur une copie encore en retard, le coffre s'ouvre ; la synchro adopte ensuite le nouvel en-tête, et une notification l'indique : « Le mot de passe maître de ce coffre a été changé sur un autre appareil. »
- PSWD Master synchronise une **copie** de la session (`Session` est `Clone`) : pendant l'échange, l'interface lit et modifie le coffre ouvert. La copie synchronisée ne remplace la session que si le coffre n'a été ni fermé ni modifié entre-temps ; sinon la modification a déjà prévu une autre synchro. Une seule synchro ou un seul échange avec le serveur à la fois : deux fils ne réservent jamais le même coffre et n'écrivent jamais la configuration ensemble. Une copie garde la clé du coffre jusqu'à la fin de son échange (2 min au plus), même si le coffre est verrouillé entre-temps.

## 8. CLI

**Commandes ponctuelles**

| Commande | Effet |
|---|---|
| `pswd init <nom>` | Crée un coffre local : mot de passe maître fort (§9, saisi 2 fois), affichage unique de la clé de récupération. **Refuse** si le nom est déjà pris. |
| `pswd open <nom>` | Déverrouille et lance le shell interactif. |
| `pswd passwd <nom>` | Change le mot de passe maître (ancien requis), refait l'enveloppe 1. |
| `pswd recover <nom>` | Clé de récupération + nouveau mot de passe, refait l'enveloppe 1. |
| `pswd import <nom> <fichier.csv>` | Importe le format CSV ci-dessous (tout le fichier est validé avant d'ajouter quoi que ce soit), puis rappelle de supprimer le fichier. |
| `pswd backup <dossier> [--vault nom]` | Sauvegarde tous les coffres (ou un seul) dans un nouveau fichier `.pswd` du dossier (§7). |
| `pswd restore <fichier.pswd> [nom]` | Restaure un coffre d'une sauvegarde comme nouveau coffre local (§7). |
| `pswd delete <nom>` | Supprime un coffre de cette machine, après avoir retapé son nom (§7). |
| `pswd connect <adresse>` | Connecte l'appareil à un serveur (§11) : clé de connexion, nom de l'appareil, nom du compte pour une clé administrateur. `hôte` seul vaut `hôte:8443`. |
| `pswd disconnect` | Déconnecte l'appareil (§11) ; demande s'il faut garder les copies locales. |
| `pswd add-device` | Affiche une clé de connexion pour un appareil de plus du même compte. |
| `pswd status` | Serveur et état de chaque coffre (§11). |
| `pswd sync` | Échange avec le serveur sans mot de passe (§7, §11) : télécharge les nouveaux coffres du compte, publie les coffres locaux, met à jour ceux dont un seul côté a changé, signale ceux à ouvrir pour fusionner, garde ceux supprimés du serveur et supprime ceux dont le compte n'est plus membre. |

Aucune commande n'écrase un coffre existant.

`init` vérifie le nom du coffre avant toute question. Un essai raté ne laisse rien derrière lui : dossier du coffre supprimé (s'il n'existait pas avant), configuration inchangée.

**Shell interactif** : `list [--tag t]`, `search <texte> [--tag t]`, `show <entrée> [--show]`, `copy <entrée>`, `add [--generate]`, `edit <entrée>`, `rm <entrée>`, `history <entrée>`, `tags`, `gen [--length N] [--no-symbols]`, `sync`, `lock`, `quit`.

- **Synchro** (serveur enregistré, §7) : à l'ouverture, après chaque commande qui modifie le coffre (aussi `passwd`, `recover`, `import`), à la fermeture (`quit`, fin de saisie) s'il reste des modifications non publiées, et par `sync`. Pas au verrouillage automatique : le thread de surveillance détruit la session, et chaque modification a déjà été synchronisée. Elle se fait au premier plan ; une synchro automatique ne s'affiche qu'en cas de problème, `sync` affiche toujours le résultat.

- **Désigner une entrée** (`show`, `copy`…) : par son titre, sans tenir compte de la casse, ou par le début de son identifiant (affiché par `list`). Si plusieurs entrées correspondent, la commande refuse et les liste. Une erreur dans le shell est affichée et le shell continue ; fin de l'entrée standard = `quit`.
- **`add` / `edit`** : questions champ par champ (une note n'a ni identifiant, ni mot de passe, ni URL) ; URLs et tags séparés par des espaces. Dans `edit`, la valeur actuelle est rappelée : réponse vide = inchangé, `-` = effacer. `--generate` crée le mot de passe sans l'afficher (`copy` pour le copier). `rm` demande confirmation (non par défaut). `history` date chaque version (UTC) et signale un mot de passe différent.
- **Presse-papiers** ouvert à la première copie : une machine sans presse-papiers peut utiliser toutes les autres commandes.
- **Verrouillage automatique** après 10 min d'inactivité : la clé est effacée de la mémoire à l'échéance, même si aucune commande n'est tapée. Un thread de surveillance détient la session ; chaque commande repousse l'échéance ; à l'échéance, il détruit la session. La saisie suivante affiche « Coffre verrouillé » et ferme le shell. `auto_lock_minutes = 0` désactive le verrouillage ; `lock` (alias de `quit`) verrouille tout de suite.
- **Copie sous Linux** : un avertissement rappelle que l'exclusion de l'historique dépend du gestionnaire de presse-papiers.
- **Recherche** : sans tenir compte de la casse, sur le titre, l'identifiant, les URLs et les tags ; les tombstones sont ignorés.
- **Générateur** : 24 caractères par défaut (8 à 24), majuscules, minuscules, chiffres et symboles `!#$%&()*+,-./:;<=>?@[]^_{|}~` (ni espace, ni guillemets, ni antislash, ni accent grave) ; au moins un caractère de chaque classe active, obtenu par rejet des candidats incomplets pour que tous les mots de passe valides restent équiprobables. `pswd-core` reçoit le générateur aléatoire en paramètre, contraint par le trait `CryptoRng` ; la CLI fournit un `StdRng` initialisé par `SysRng` (aléa du système, `rand` 0.10) : si l'aléa manque, une erreur est affichée, jamais de panique.
- **Affichage** : mot de passe masqué par défaut ; le mot de passe maître n'est jamais copié, et n'est affiché que pendant sa saisie, à la demande de l'utilisateur (icône en forme d'œil).

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

- Secrets (`password`, clés, mot de passe maître, JSON déchiffré) dans des types effacés à la libération (`Zeroizing` de `zeroize`, `Secret` de `pswd-core`) ; jamais dans les logs, `Debug` ni les messages d'erreur.
- **Presse-papiers** : seulement les mots de passe d'entrées ; effacement après 20 s, ou dès la fermeture du programme si elle survient avant, uniquement s'il contient encore la valeur copiée (sauf sur Android quand il est illisible, §12) ; seul le minuteur de la dernière copie efface. Le contenu doit être exclu de l'historique dès la copie :
  - Windows : formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory = 0`, `CanUploadToCloudClipboard = 0` ;
  - Linux : type MIME `x-kde-passwordManagerHint = secret` (partiel : avertissement affiché) ;
  - Android 13+ : `EXTRA_IS_SENSITIVE` (§12) ; iOS (à faire sur le Mac) : `localOnly` + `expirationDate`.
  - **Fermeture sur le bureau** : Tauri termine le programme sans libérer son état, donc sans exécuter les destructeurs. L'effacement est explicite (`SecureClipboard::clear_now`, appelé sur `RunEvent::Exit`). Sous Linux, il évite aussi de confier la copie au gestionnaire de presse-papiers, qui reprend le contenu d'un programme qui se ferme. Limite : un programme tué (gestionnaire des tâches, plantage, coupure) n'efface rien, et son minuteur disparaît avec lui.
- **Mot de passe maître** : tout nouveau mot de passe (création, `passwd`, `recover`, et leurs équivalents graphiques) doit avoir au moins 12 caractères (comptés en caractères, pas en octets) et mélanger au moins 3 types parmi minuscules, majuscules, chiffres et symboles (tout autre caractère : ponctuation, espace, lettre sans casse). La règle vit dans `pswd-app` (`check_master_password`) ; l'interface la refait avant l'envoi (`validation.ts`) et l'affiche sous le champ. Un mot de passe déjà en place n'est jamais revérifié : il ouvre toujours son coffre.
- Mot de passe maître **et** clé de récupération perdus = coffre perdu. Aucune récupération possible côté serveur.
- **Anciennes copies** : après `passwd` / `recover`, l'ancien mot de passe ouvre toujours les copies antérieures du fichier (sauvegardes externes, versions conservées par le serveur). Si le mot de passe a fuité, une rotation de clé est nécessaire (hors feuille de route).
- **Interface graphique** : JavaScript ne peut pas effacer une chaîne de la mémoire ; le moins de secrets possible traversent vers la webview (§12).
  - Jamais côté JS : clé du coffre, coffre déchiffré, mot de passe copié (la copie est faite par Rust).
  - Côté JS seulement sur demande explicite : mot de passe affiché (« Afficher », masqué à nouveau après 30 s), mot de passe généré affiché.
  - **Limite acceptée** : les saisies (mot de passe maître, mot de passe d'une entrée) passent par un champ HTML, donc par JS ; le champ est vidé dès l'envoi.
  - CSP stricte (ressources de l'application seulement, ni script externe, ni `eval`, ni réseau) ; capacités Tauri limitées à nos commandes (aucun accès fichiers ou shell depuis JS) ; les adresses d'une entrée ne sont pas des liens pour l'instant (ouverture reportée, §2) : elles s'ouvriront dans le navigateur du système, jamais dans la webview.
- **Mobile** : écran masqué dans les applications récentes et captures interdites (Android `FLAG_SECURE`, écran neutre sur iOS).

## 10. Points d'extension à préparer

Chaque point est marqué dans le code par un commentaire `// EXTENSION(<nom>): …` à l'endroit exact où il se branchera, dès que cet endroit existe.

| Nom | Préparation |
|---|---|
| `keyfile` | `kind = 3` réservé dans les enveloppes ; dérivation de la clé d'enveloppe isolée pour accepter un second secret ; port de déverrouillage générique. |
| `key-rotation` | Les enveloppes sont indépendantes du payload ; `pswd passwd --rotate-key` génèrera une nouvelle clé de coffre et rechiffrera. |
| `mobile` | `pswd-core` sans E/S ; port `SecureClipboard` réimplémenté par un plugin Tauri maison : Kotlin fait, Swift à faire sur le Mac (§12). |
| `biometrie` | Déverrouillage par empreinte ou Face ID : une clé rangée dans le stockage sécurisé du téléphone (Keystore, Keychain), à concevoir à part. |
| `anglais` | Textes de l'interface regroupés par langue (`src/lib/i18n/fr.ts`) ; `en.ts` devra avoir les mêmes clés (vérifié par TypeScript). Les erreurs Rust remontent à l'interface sous forme de codes, traduits côté interface. |
| `remote-transport` | `RemoteStore` abstrait : l'adaptateur HTTP de `pswd-server` s'y branche (étape 3). |
| `partage-par-membre` | Plusieurs enveloppes possibles par coffre (une par membre, à clé publique) : ajouter ou retirer un membre sans partager de mot de passe. Les coffres communs de l'étape 3 utilisent un mot de passe partagé. |
| `dossier-reseau` | Synchro par un partage réseau (NAS) sans `pswd-server`. Un dossier ne sait pas refuser une écriture concurrente : il faudrait un fichier par appareil, fusionnés à la lecture. |
| `multi-serveurs` | Un appareil connecté à plusieurs serveurs : chaque coffre est rattaché à un serveur. |
| `cert-rotation` | Changer le certificat du serveur (clé privée volée) : les appareils devraient retenir la nouvelle empreinte sans se reconnecter un par un. |

## 11. Serveur maison (étape 3)

**Principe** : `pswd-server` est un exécutable Rust unique, sans dépendance à installer, lancé sur une machine de l'utilisateur. Cible : un PC sous **Ubuntu Server 26.04 LTS** (x86_64), en service `systemd` ; essayé d'abord dans une machine virtuelle Ubuntu Server 26.04 (VirtualBox ou VMware, réseau en pont) avant le vrai déploiement. C'est une passerelle : il stocke des fichiers `vault.enc` opaques et ne détient aucune clé. Toute la gestion se fait depuis PSWD Master.

**Installation** : un paquet `.deb` (`cargo-deb`), installé par `sudo apt install ./pswd-server_<version>_amd64.deb`, sans aucune étape manuelle. Il installe `/usr/bin/pswd-server`, crée l'utilisateur système `pswd-server` (sans connexion ; le serveur ne tourne jamais en `root`) et le dossier `/var/lib/pswd-server/` (base et certificat, accessibles à ce seul utilisateur), affiche l'adresse (première adresse de `hostname -I`, port de la configuration) et la clé administrateur, puis installe et démarre le service. À la première installation, la base, le certificat et la clé sont créés par `reset-admin` (lancé en `pswd-server` par `runuser`) **avant** le démarrage du service, qui ne doit pas créer le certificat en même temps. Configuration : `/etc/pswd-server/config.toml` (`port`, 8443 par défaut ; fichier absent : valeurs par défaut) ; le service écoute sur toutes les interfaces à ce port. Une mise à jour (nouveau `.deb`) et `apt remove` conservent les données ; seul `apt purge` les efface (avec l'utilisateur système). Le service `systemd` (`debian/pswd-server.service`) tourne en `pswd-server`, n'écrit que dans `/var/lib/pswd-server` (`ProtectSystem=strict`) et n'a aucun privilège (`CapabilityBoundingSet=` vide, `NoNewPrivileges`). Fichiers du paquet dans `crates/pswd-server/debian/`, toujours en fins de ligne LF (`.gitattributes`). Guide pas à pas : `docs/SERVEUR.md`.

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
| `fetch()` | `GET /v1/vaults/{id}` | `200` + octets + `ETag: "<version>"` ; `204` jamais publié (`None`) ; `404` inconnu ; `410` + `{"reason": "deleted"}` coffre supprimé du serveur, ou `{"reason": "left"}` le compte n'en est plus membre |
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
| `DELETE /v1/vaults/{id}` | membre | `204` : le propriétaire supprime son coffre privé (effet ci-dessous) ; `400` coffre commun, qui se quitte |

Routes d'administration (`403` pour un appareil dont le compte n'est pas administrateur ; dates en millisecondes depuis 1970) :

| Requête | Effet |
|---|---|
| `GET /v1/admin/accounts` | `200 [{id, name, is_admin, devices: [{id, name, created_at}]}]`, triés par nom |
| `DELETE /v1/admin/accounts/{id}` | `204` : compte supprimé avec ses appareils, ses clés et les invitations qu'il a reçues ou envoyées ; ses coffres restent sur le serveur, ceux dont il était le seul membre n'en ont plus ; `404` ; `409` dernier administrateur |
| `PUT /v1/admin/accounts/{id}/admin` `{is_admin}` | `204` ; `404` ; `409` retirer son rôle au dernier administrateur. Le rôle est relu à chaque requête : le changement vaut aussitôt |
| `DELETE /v1/admin/devices/{id}` | `204` : appareil révoqué ; `404` |
| `GET /v1/admin/keys` | `200 [{id, purpose, name, expires_at}]` : clés de connexion inutilisées et valables ; `purpose` = `admin`, `member` (`name` : compte à créer) ou `device` (`name` : compte de l'appareil) |
| `DELETE /v1/admin/keys/{id}` | `204` : clé annulée ; `404` |
| `GET /v1/admin/vaults` | `200 [{id, name, shared, version, members: [{id, name}]}]` : tous les coffres non supprimés, triés par nom ; `version` entière ou `null` |
| `DELETE /v1/admin/vaults/{id}` | `204` : coffre supprimé (contenu, versions et invitations effacés ; ses membres en gardent la trace et reçoivent `410`) ; `404` |
| `POST /v1/admin/vaults/{id}/members` `{account_id}` | `204` : rattache un compte à un coffre sans membre ; `400` compte inconnu ; `404` ; `409` le coffre a encore des membres |
| `DELETE /v1/admin/vaults/{id}/members/{account}` | `204` : membre retiré d'un coffre commun (il reçoit `410`, raison `left`) ; `400` coffre privé, qui n'a que son propriétaire et se supprime ; `404` |
| `GET /v1/admin/vaults/{id}/versions` | `200 [{version, created_at, size}]`, de la plus récente à la plus ancienne ; `404` |
| `POST /v1/admin/vaults/{id}/versions/{v}/restore` | `201 {id, name}` : nouveau coffre `<nom>-v<v>` (nom raccourci pour tenir en 64 caractères), même type, mêmes membres, publié en version 1 ; les appareils le reçoivent au prochain échange ; `404` |
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

**Accès** : l'application retient une adresse locale et une adresse publique, essayées dans cet ordre (délai max 5 s). L'adresse publique (nom DNS ou adresse IP, port facultatif) est renseignée par un administrateur dans le panneau ; le serveur ne la devine pas (il est derrière la box). Chaque appareil la reçoit à chaque échange réussi (`GET /v1/server`) et met `server.toml` à jour. Un changement d'adresse est sans risque : le certificat est vérifié quelle que soit l'adresse. Un appareil hors de chez soi qui ne joint plus l'ancienne adresse la reçoit dès qu'il repasse par l'adresse locale : un nom DNS dynamique est conseillé plutôt qu'une adresse IP qui change. L'accès depuis l'extérieur (redirection de port, VPN) est documenté ; il n'y a pas de relais.

**Administration**
- À l'installation, le serveur affiche une fois son adresse et une **clé administrateur** (clé de connexion, ci-dessous). Le premier appareil qui la présente devient administrateur ; personne d'autre ne le peut.
- Secours : sur la machine, `pswd-server reset-admin` affiche une nouvelle clé administrateur (aussi si la première a expiré) et annule les précédentes. Présentée, elle crée un **nouveau** compte administrateur ; l'ancien compte reste, à supprimer depuis le panneau. Le serveur se lance par `pswd-server serve` (`--config`, par défaut `/etc/pswd-server/config.toml` ; `--listen` remplace l'écoute sur toutes les interfaces, par exemple `127.0.0.1:8443` en développement) ; toutes les sous-commandes (`serve`, `reset-admin`, `backup`, `restore`) prennent `--data-dir` (par défaut `/var/lib/pswd-server`). Les noms datés des sauvegardes (`.pswd` et serveur) viennent de `pswd-core` (`utc.rs`), partagé par l'application et le serveur.
- Le panneau d'administration de PSWD Master permet d'ajouter un membre, de supprimer un compte, de révoquer un appareil, de promouvoir un membre administrateur ou de lui retirer ce rôle, de supprimer un coffre, de rattacher quelqu'un à un coffre sans membre, de restaurer une version, de renseigner l'adresse publique et de sauvegarder le serveur. Le dernier administrateur ne peut pas perdre son rôle ni être supprimé. Il voit aussi les clés de connexion en attente (usage, expiration) et peut les annuler. Restaurer une version sert à rattraper une erreur que la fusion a propagée partout (entrée supprimée, mot de passe d'un coffre commun changé puis oublié) : la restauration crée un nouveau coffre, car remplacer le contenu serait aussitôt défait par la fusion avec les copies des appareils.
- Le panneau s'ouvre depuis l'écran « Réglages » (bouton « Administration ») ; le bouton n'apparaît que si le serveur répond que le compte est administrateur. L'écran « Serveur » montre le compte de l'appareil. Quatre onglets : Comptes (appareils, rôle, suppression, ajout d'un membre avec sa clé montrée une fois), Coffres (membres, rattachement, versions et restauration, suppression), Clés (en attente, annulation), Serveur (adresse publique, sauvegarde). Supprimer un compte ou un coffre du serveur se confirme en retapant son nom.

**Comptes et appareils**
- Ajouter un membre produit l'adresse du serveur et une **clé de connexion**, à recopier dans l'application (ordinateur ou mobile). Même format que la clé de récupération (§5 : Base32 Crockford, 52 caractères en 13 groupes, même règle de saisie), portant 32 octets : 16 octets de secret aléatoire, puis les 16 premiers octets de l'empreinte du certificat. L'application vérifie que le certificat présenté commence par ces octets, puis retient l'empreinte complète : le serveur est vérifié dès la première connexion, sans comparaison manuelle.
- Une clé de connexion est à usage unique, valable 7 jours, annulable depuis le panneau. Le même format sert pour la clé administrateur, l'ajout d'un membre et l'ajout d'un appareil à un compte existant (clé générée depuis un appareil déjà connecté du compte).
- Chaque appareil reçoit son propre jeton, révocable individuellement. Le serveur ne garde que l'empreinte (SHA-256) des jetons et des clés de connexion : une copie de la base ne donne aucun accès. Le compte n'a pas de mot de passe serveur : rien ne se confond avec le mot de passe maître.
- Sur l'appareil, l'adresse locale, l'adresse publique, l'empreinte du certificat et le jeton sont rangés dans un fichier, `<data_dir>/pswd/server.toml` (`local_address`, `public_address` facultative, `fingerprint` et `token` en hexadécimal ; droits `0600` sous Linux ; dossier privé de l'application sur mobile). Son existence signifie qu'un serveur est **enregistré** ; l'appareil n'est dit **connecté** que si son dernier contact avec ce serveur a réussi. Pas de trousseau du système : il ne protègerait pas mieux (le `vault.enc` chiffré, que le jeton permet de télécharger, est dans le même dossier), il manque souvent sous Linux sans bureau et exigerait du code natif sur mobile. Un jeton volé se révoque depuis le panneau.
- Un appareil est connecté à un seul serveur. Il peut s'en déconnecter (son jeton est révoqué) puis se connecter à nouveau. En se connectant, il télécharge la copie chiffrée de tous les coffres publiés dont le compte est membre et qu'il n'a pas déjà ; chacun se synchronise ensuite à son ouverture (§7). Les coffres créés plus tard par un autre appareil arrivent par le même échange, relancé automatiquement par PSWD Master (§7) ou à la demande (`pswd sync`, bouton « Synchroniser »).
- Un coffre téléchargé prend le nom donné par le serveur ; s'il est pris sur l'appareil, `<nom>-<8 premiers caractères de l'identifiant>` (nom raccourci pour tenir en 64 caractères) ; s'il est pris à son tour, le coffre n'est pas téléchargé et le problème est signalé.
- Le lien d'un coffre avec le serveur est noté dans `config.toml` : `[vaults.<nom>.remote]`, avec `server` (empreinte du certificat, en hexadécimal) et `id` (identifiant sur le serveur). Il est noté dès la réservation, avant la publication.
- **CLI** : se connecter, se déconnecter, synchroniser, générer une clé pour ajouter un appareil à son compte. Coffres communs (inviter, accepter, quitter) et administration se font seulement dans PSWD Master ; un coffre commun déjà accepté se synchronise normalement dans la CLI.

**Coffres**
- Un coffre est **privé** ou **commun**, choisi à sa création dans PSWD Master et jamais changé : le mot de passe d'un coffre privé n'a jamais à être transmis. La CLI ne crée que des coffres privés. Un coffre commun est protégé par un mot de passe maître partagé, transmis hors du serveur. Le serveur garde ce type (`shared`), donné à la réservation ; un coffre local retient le sien dans `config.toml` (`shared = true`) jusqu'à sa publication.
- Tout membre d'un coffre commun peut inviter un utilisateur du même serveur, choisi dans la liste des comptes, et annuler une invitation en attente. L'invité voit ses invitations en haut de la liste des coffres (demandées au serveur à chaque échange : connexion, « Synchroniser », échange automatique) ; il accepte ou refuse. Accepter le rend membre : le coffre est téléchargé aussitôt, et s'ouvre avec le mot de passe que lui donne celui qui l'a invité. Tout membre peut quitter le coffre. Tous les membres sont égaux ; seul un administrateur retire quelqu'un. Après un départ, changer le mot de passe (`passwd`) est conseillé : l'ancien membre le connaît.
- Un compte qui a quitté un coffre, ou en a été retiré, en garde une trace sur le serveur : ses appareils reçoivent `410` (raison `left`), et non `404`, et suppriment leur copie locale au prochain échange ou à la prochaine synchro (modifications non publiées comprises). Une nouvelle invitation acceptée efface cette trace : le coffre est de nouveau téléchargé.
- Être membre donne le droit de télécharger et de publier le fichier ; le lire exige le mot de passe du coffre.
- Le serveur ne supprime jamais un coffre de lui-même, même sans membre. Depuis l'application, supprimer un coffre privé publié propose « local seulement » (par défaut) ou « local et serveur » ; un coffre commun se quitte. Un coffre déjà supprimé du serveur par un autre appareil est alors seulement supprimé de l'appareil. Supprimer un coffre sur le serveur ne touche pas aux copies locales des appareils.
- Une suppression sur le serveur (nom du coffre à retaper) efface aussitôt son contenu et ses versions ; seule reste une trace (identifiant, « supprimé ») : ses anciens membres reçoivent `410` (raison `deleted`) et gardent leur copie locale. Pas de corbeille : les copies locales et les sauvegardes suffisent à rattraper une erreur.
- Le serveur conserve les **15 dernières versions** de chaque coffre. Une restauration crée un nouveau coffre avec les mêmes membres (§6).

**État d'un coffre sur un appareil** (calculé à partir de la liste des coffres du serveur, `Vaults::vault_states` ; aucun état sans connexion ou si le serveur est injoignable)

Règle des couleurs : **rouge** dès que le coffre n'a pas de lien avec le serveur (jamais publié, lié à un autre serveur, supprimé du serveur), même s'il est gardé sur l'appareil ; **orange** quand le lien existe mais que les deux copies diffèrent (nuance : rien n'est perdu, la synchro peut les réunir) ; **verte** quand elles sont identiques.

| État | Pastille | Signification | Comportement |
|---|---|---|---|
| local | rouge | Pas sur ce serveur : jamais publié, ou lié à un autre serveur | Publié automatiquement, tel quel et sans mot de passe, à chaque échange et à chaque synchro (le lien avec un autre serveur est remplacé) |
| à jour | verte | Même contenu que la copie du serveur | — |
| à mettre à jour | orange | Un seul côté a changé depuis l'état commun | Mis à jour sans mot de passe au prochain échange (§7) |
| à déverrouiller | orange | Modifié des deux côtés, ou état commun inconnu | Fusionné à son ouverture ; l'échange le signale |
| supprimé | rouge | Supprimé du serveur (`410`, raison `deleted`) | Gardé en local, noté `removed = true` dans `[vaults.<nom>.remote]`, jamais republié automatiquement : seulement par le bouton « Synchroniser » du coffre ouvert (commande `sync` du shell), comme nouveau coffre |

- Un coffre commun dont le compte n'est plus membre (il l'a quitté depuis un appareil, ou un administrateur l'en a retiré) n'a pas d'état : il est supprimé de chaque appareil du compte (`SyncOutcome::Left`). Quitter un coffre le supprime aussitôt de l'appareil.
- **Limite acceptée** : un coffre republié sur un nouveau serveur oublie l'ancien ; reconnecté plus tard à l'ancien serveur, l'appareil y publierait un doublon.
- À la déconnexion, l'application demande s'il faut garder une copie locale des coffres (oui par défaut). Sans copie, les données restent sur le serveur : seuls sont supprimés de l'appareil les coffres dont l'appareil n'a rien que le serveur n'ait déjà : le coffre figure encore dans la liste du serveur, et l'empreinte de la copie locale est celle du dernier état commun (`synced.file_hash`). Le serveur a pu avancer depuis : il ne fait qu'ajouter à cet état, il n'y a donc rien à perdre. Les autres (modifications non publiées, jamais synchronisé, absent du serveur, serveur injoignable) sont gardés et signalés.
- Déconnexion : vérification des copies (si demandée), révocation du jeton, suppression de `server.toml`, puis des copies locales. Un serveur injoignable n'empêche pas de se déconnecter : le jeton, non révoqué, est signalé, à révoquer depuis le panneau.
- Le serveur distingue « coffre inconnu » de « coffre retiré » : `RemoteStore` reçoit pour cela une erreur dédiée ; l'adaptateur HTTP retient en plus la raison du `410` (`RemoteVault::removal`) et la dernière version vue (`RemoteVault::version`).

## 12. Interfaces graphiques (étape 2)

**Application** : PSWD Master, identifiant `fr.arthurfontaine.pswdmaster` (modifiable jusqu'à une première publication sur un store). Un seul projet Tauri 2 pour Windows, Linux, macOS, Android et iOS ; une seule branche `dev-interface`. Les différences entre plateformes se règlent dans le code (`#[cfg(target_os = …)]`), jamais par des branches.

**Organisation** : `crates/pswd-gui/` contient l'interface (`package.json`, `src/`, Vite) et `crates/pswd-gui/src-tauri/` la crate Rust `pswd-gui` (membre du workspace), avec `tauri.conf.json`, `capabilities/` et le projet Android généré (`gen/android`). Les commandes appelables sont déclarées dans `build.rs` et autorisées une à une dans `capabilities/default.json` ; un test vérifie qu'une commande existante mais non autorisée est refusée.

**Fonctionnement** : la webview du système affiche l'interface (HTML/CSS/JS intégrés à l'exécutable, aucun serveur web, aucun réseau). L'interface appelle les commandes Tauri du backend Rust, qui s'appuie sur `pswd-app`. La session (clé du coffre, coffre déchiffré) reste en Rust. Règles de sécurité : §9.

**Interface** : Svelte 5 + TypeScript, construite par Vite ; CSS maison avec des variables (couleurs, espacements), sans bibliothèque de composants. Police Atkinson Hyperlegible Next (paquet `@fontsource-variable`, embarquée : aucun réseau), choisie pour sa lisibilité (l, I et 1, O et 0 distincts).
- Thème clair ou sombre selon le système, réglable (auto, clair, sombre). C'est le seul réglage de l'écran « Réglages » : les délais (verrouillage, presse-papiers, arrière-plan) gardent leurs valeurs par défaut, modifiables à la main dans `config.toml` sur le bureau.
- Mise en page de départ, appelée à évoluer : deux colonnes sur le bureau (liste à gauche, détail à droite), une colonne sur mobile (liste, puis détail en plein écran). Sur un très grand écran (à partir de 1600 × 896 px), la taille de base passe à 18 px, puis 20 px (à partir de 2240 × 1200 px) : tout grandit ; le contenu s'élargit et, hors coffre ouvert, descend vers le centre. Sur l'écran d'accueil, les cartes des coffres ont toutes la même taille : un nom trop long est coupé par « … » (le nom complet reste au survol et à l'ouverture).
- Tout champ de mot de passe (entrée, mot de passe maître) a une icône en forme d'œil qui l'affiche en clair ou le masque de nouveau.
- Le numéro de version (`v0.1.0`) s'affiche en petit à droite du titre ; il vient de `tauri.conf.json`, lu par Vite à la construction (`__APP_VERSION__`). La version est aussi dans `Cargo.toml` et `package.json` : les trois se changent ensemble.
- Les confirmations (entrée enregistrée, copie, import, sauvegarde, restauration, suppression…) s'affichent dans une notification passagère (`src/lib/toast.svelte.ts`) ; les erreurs restent dans l'écran concerné.
- Bureau entièrement utilisable au clavier (Tab, Entrée, Échap, `Ctrl+F`) ; contrastes suffisants.
- Tactile : cibles d'au moins 48 px sur un écran tactile (`pointer: coarse`), survol réservé aux pointeurs qui survolent, marges des zones sûres (affichage bord à bord d'Android 15+, encoche) ; les champs saisis tels quels (nom de coffre, identifiant, clé de récupération, adresses, tags, recherche) désactivent majuscule et correction automatiques (`src/lib/keyboard.ts`).
- Bouton retour d'Android (`src/lib/back.ts`) : chaque écran ouvert s'inscrit, le dernier répond en premier. Il referme le formulaire ou le panneau, puis l'entrée ; dans la liste des entrées, il verrouille le coffre ; ailleurs, il revient à la liste des coffres, d'où il ferme l'application. La clé de récupération ne se quitte que par « Continuer ». Permissions dans `capabilities/mobile.json`, limitée à Android (écoute du bouton, fermeture) ; un test vérifie qu'elle ne s'applique pas au bureau.
- Français d'abord ; tous les textes passent par `src/lib/i18n/<langue>.ts` (cf. `EXTENSION(anglais)`).

**Fichiers et dossiers**
- Bureau : les mêmes dossiers que la CLI (`Locations::system`) ; les deux programmes voient les mêmes coffres et peuvent modifier le même à la fois : chaque écriture fusionne d'abord la copie sur le disque (§3, Session). Un programme ne voit les modifications de l'autre qu'à sa prochaine écriture ou réouverture. **Limite acceptée** : sans verrou de fichier, deux écritures dans la même fraction de seconde peuvent encore se recouvrir.
- Mobile : le dossier privé de l'application fourni par Tauri (`app_data_dir`), passé à `Locations::new` ; `FileStore` fonctionne tel quel.
- Sauvegarde : un fichier `.pswd` (§7), sur le bureau comme sur mobile. « Sauvegarder » liste les coffres de l'appareil, chacun avec une case à cocher (et « Tout sélectionner », aucune cochée au départ) ; la confirmation ouvre la fenêtre système d'enregistrement et écrit les coffres cochés dans un seul fichier. « Restaurer » ouvre le sélecteur de fichiers et liste les coffres du fichier, chacun avec une case à cocher (et « Tout sélectionner ») ; un bouton de confirmation restaure les coffres cochés un par un selon les règles du §7. Si l'un échoue, ceux déjà restaurés le restent et quittent la liste. `pswd-app` restaure à partir des octets du fichier, pas d'un chemin. Les fichiers choisis sont lus et écrits par Rust (module `files`), jamais par l'interface ; une écriture se termine par `sync_all`.
- Les fenêtres système du mobile renvoient une adresse (`content://` sur Android), pas un chemin : Rust ouvre les fichiers choisis par le plugin `fs` (`FsExt::open`), qui accepte les deux. L'interface n'a aucune permission de ce plugin (vérifié par un test). L'interface n'affiche jamais un chemin complet : la sauvegarde donne seulement le nombre de coffres, la restauration le nom du fichier `.pswd` quand l'adresse le contient (rien sinon). **À vérifier sur le Mac** : sur iOS, le plugin ouvre un accès « security-scoped » à chaque fichier, qu'il faudrait refermer.

**Consultation** : la liste et le détail ne contiennent jamais de mot de passe. « Copier » est fait par Rust (`copy_password`) ; « Afficher » (`reveal_password`) envoie le mot de passe à l'interface, qui le masque de nouveau au bout de 30 s, ou dès qu'on change d'entrée. Recherche à chaque frappe (côté Rust, `pswd_app::search`), filtre par tag, `Ctrl+F` pour aller à la recherche. Sur le bureau, la liste est découpée en pages qui tiennent dans la fenêtre sans défiler (`src/lib/layout.ts`) ; une nouvelle recherche revient à la première page. Le menu « Gérer le coffre » donne accès au changement de mot de passe maître, à l'import CSV et à la suppression du coffre. Toute lecture du coffre repousse son verrouillage ; si le coffre s'est verrouillé entre-temps, l'interface revient à la liste.

**Modification** : un formulaire unique pour ajouter et modifier. Une entrée est un « Mot de passe » (`login`) ou une « Note sécurisée » (`note`) ; une note n'a ni identifiant, ni mot de passe, ni URL ; URLs une par ligne, tags séparés par des espaces ou des virgules). Pour le mot de passe : « Saisir » (le champ est vidé dès l'envoi), « Générer » (longueur 8 à 24, symboles ou non : **tiré en Rust à l'enregistrement**, il ne passe jamais par l'interface), et en modification « Inchangé » (par défaut) ou « Supprimer ». La suppression d'une entrée demande une confirmation dans l'écran. L'historique liste les versions précédentes (date, titre, identifiant, « mot de passe différent »), jamais les anciens mots de passe.

**Écran masqué** (§9) : sur Android, `FLAG_SECURE` est posé dans `MainActivity.kt` (`gen/android`, versionné ; à reporter si le projet Android est régénéré) : écran noir dans les applications récentes, captures et enregistrements refusés. Sur iOS, une vue neutre recouvrira l'interface dès que l'application quitte le premier plan ; elle sera ajoutée au projet Xcode généré sur le Mac. Vérifié à la main sur appareil.

**Presse-papiers** : sur le bureau, `LazyClipboard` de `pswd-infra` (ouvert à la première copie, partagé avec la CLI). Sur mobile, un plugin maison implémente `SecureClipboard` avec les marqueurs du §9 ; le plugin officiel ne les pose pas.
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
- Écran « Serveur », ouvert par un bouton de la barre du haut. Aucun serveur enregistré : formulaire de connexion (adresse, clé de connexion, nom de l'appareil, nom du compte, ce dernier seulement avec la clé administrateur). La clé est effacée du champ dès l'envoi ; une fois connecté, l'appareil échange aussitôt ses coffres avec le serveur, et un échange incomplet est signalé. Serveur enregistré : « Connecté au serveur X. » si le dernier contact a réussi ; sinon « Serveur enregistré : X. » suivi de « Échanges impossibles depuis HH:MM : <raison>. » (heure du premier échec de la série, retenue en mémoire par le backend et effacée au premier contact réussi ; le compte n'est alors pas demandé). Dans les deux cas : « Synchroniser » (échange sans mot de passe, §7), « Ajouter un appareil » (clé de connexion montrée une seule fois, à la demande de l'utilisateur, puis oubliée) et « Se déconnecter » (case « Garder une copie des coffres sur cet appareil », cochée par défaut).
- Liste des coffres, si un serveur est enregistré : pastille d'état sur la carte du coffre (§11) et bouton « Synchroniser » (échange sans mot de passe). Échange automatique et muet (§7) : au montage de la liste, au retour au premier plan (`visibilitychange`, et `focus` de la fenêtre sur ordinateur) et toutes les 30 s tant que la liste est affichée et la page visible ; le minuteur s'arrête quand la liste disparaît. Une seule commande, `refresh_server`, fait l'échange et renvoie l'état des coffres (`null` si un échange est déjà en cours) ; un serveur qui n'a pas répondu n'est pas recontacté pour lire cet état, ni pour les invitations. Les cartes ne sont redessinées que si les noms des coffres ont changé. Les erreurs ne sont pas affichées (l'écran Serveur indique depuis quand les échanges échouent) ; le bouton « Synchroniser » les montre. Un coffre supprimé du serveur l'explique dans son formulaire de déverrouillage, puis une fois ouvert.
- Coffre ouvert, si un serveur est enregistré : bouton « Synchroniser » (synchro avec fusion ; republie un coffre supprimé du serveur). Vérification toutes les 30 s tant que la page est visible (§7) ; une synchro qu'elle déclenche se signale comme les autres synchros automatiques. Un coffre dont le compte n'est plus membre est fermé et supprimé. Les synchros automatiques se font en Rust, dans un autre fil ; à la fin de chacune, Rust envoie l'événement `vault-synced`. Un problème (hors ligne, conflit, copie du serveur refusée, erreur locale) reste affiché dans un bandeau jusqu'à la synchro réussie suivante ; une synchro qui apporte des modifications fait relire la liste.
- Les commandes qui parlent au serveur s'exécutent hors du fil de l'interface (commandes `async`, `spawn_blocking`). La suppression d'un coffre aussi : elle attend la fin d'une synchro en cours, qui pourrait réécrire le coffre.
- Les messages d'erreur du serveur (« serveur injoignable »…) viennent de Rust, en français, comme détail d'un code (`serverError`) ou d'un résultat par coffre (cf. `EXTENSION(anglais)`).

**Tests**
- `pswd-app` et le backend de `pswd-gui` : 100 % lignes et branches visées, comme `pswd-infra` ; exclusions justifiées (démarrage de Tauri, appel au plugin natif). Les commandes sont testées avec le moteur factice de Tauri (`tauri::test`) et la vraie configuration, capacités comprises.
- Sous Windows, le manifeste de l'application (contrôles communs v6) est lié à tous les exécutables par `build.rs`, sinon les tests ne démarrent pas.
- Kotlin et Swift : réduits au minimum, testés à la main sur un appareil réel.
- TypeScript : Vitest, 100 % des lignes pour la logique (`.ts`) ; composants Svelte testés par comportement avec Testing Library, sans seuil.
- `svelte-check` et ESLint strict sans avertissement ; `package-lock.json` versionné et `npm audit` sans faille, comme `cargo deny`. `npm audit` est toujours lancé en entier (sans `--audit-level`) ; une faille n'est acceptée que si aucun correctif n'existe et qu'elle n'est pas exploitable, et elle est listée ici :
  - `deepmerge` 4.3.1 (GHSA-gcqx-w48m-w2h4, gravité faible), par `@sveltejs/vite-plugin-svelte` : empoisonnement de prototype de l'objet fusionné. Seuls nos fichiers de configuration y passent, à la construction ; la bibliothèque n'est pas dans l'application livrée. Accepté le 2026-10-28, à revoir à chaque mise à jour du plugin.
- Pas de test de bout en bout (`tauri-driver` : Windows et Linux seulement) : l'application construite est essayée à la main.
- Builds mobiles lancés à la main : Android à la fin de chaque sous-étape qui touche au mobile ; iOS sur le Mac, quand il sera disponible.

**Installateurs** (`npm run tauri build`, dans `crates/pswd-gui`, sur chaque système) : NSIS sous Windows (installation pour l'utilisateur courant, sans droits d'administrateur, en français ; WebView2 téléchargé si absent), `.deb` et AppImage sous Linux, `.app` et `.dmg` sous macOS. Pas de MSI : l'installateur NSIS suffit et n'exige pas WiX. Les installateurs ne sont pas signés (avertissement SmartScreen ou Gatekeeper à la première ouverture) ; pas de mise à jour automatique, qui demanderait un serveur de publication. La CLI s'installe à part avec `cargo install` (`docs/INSTALLATION.md`). Android (APK signé) : construit et testé à la main sur un téléphone ; iOS : sur le Mac, plus tard. L'APK de release (`npm run tauri android build -- --apk`) est signé par Gradle (`app/build.gradle.kts`) avec la clé décrite dans `gen/android/keystore.properties`, ignoré par git, la clé elle-même restant hors du dépôt (`docs/INSTALLATION.md`) ; sans ce fichier, l'APK sort non signé.
