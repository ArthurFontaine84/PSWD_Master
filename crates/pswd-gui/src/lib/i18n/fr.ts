import type { KeyPurpose } from "../api";

/** Message construit à partir du détail d'une erreur (qui peut être ignoré). */
type Message = (detail: string) => string;

const day = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long" });
const dateTime = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "short" });
const time = new Intl.DateTimeFormat("fr-FR", { timeStyle: "short" });

const errors = {
  configInvalid: (detail: string) => `Configuration invalide : ${detail}`,
  configUnreadable: (detail: string) => `Configuration illisible : ${detail}`,
  unknownVault: (detail: string) => `Coffre inconnu : ${detail}.`,
  vaultExists: (detail: string) => `Un coffre nommé ${detail} existe déjà.`,
  invalidVaultName: (detail: string) =>
    `Nom de coffre invalide : « ${detail} » (32 caractères au plus, sans espace au début ni à la fin).`,
  passwordTooShort: () => "Le mot de passe maître doit contenir au moins 12 caractères.",
  passwordTooSimple: () =>
    "Le mot de passe maître doit mélanger au moins 3 types de caractères parmi : minuscules, majuscules, chiffres, symboles.",
  wrongPassword: () => "Mot de passe maître incorrect.",
  vaultFolderExists: () => "Un dossier de coffre existe déjà à cet emplacement.",
  vaultUnreadable: (detail: string) => `Coffre illisible : ${detail}`,
  vaultLocked: () => "Le coffre est verrouillé.",
  entryNotFound: () => "Cette entrée n'existe plus.",
  noPassword: (detail: string) => `L'entrée « ${detail} » n'a pas de mot de passe.`,
  clipboard: (detail: string) => `Copie impossible : ${detail}`,
  invalidUrl: () => "Adresse non ouverte : seules les adresses web (http ou https) s'ouvrent dans le navigateur.",
  urlNotOpened: (detail: string) => `Ouverture du navigateur impossible : ${detail}`,
  emptyTitle: () => "Le titre ne peut pas être vide.",
  invalidLength: () => "La longueur doit être comprise entre 8 et 24.",
  saveFailed: (detail: string) => `Enregistrement impossible : ${detail}`,
  wrongRecoveryKey: () => "Clé de récupération incorrecte.",
  invalidRecoveryKey: () => "Clé de récupération mal saisie : 13 groupes de 4 caractères séparés par des tirets.",
  confirmationMismatch: () => "Le nom retapé ne correspond pas.",
  fileUnreadable: (detail: string) => `Fichier illisible : ${detail}`,
  csvInvalidHeader: () =>
    "Première ligne du CSV attendue : title,kind,username,password,urls,notes,tags",
  csvFieldCount: (detail: string) => `CSV, ligne ${detail} : nombre de colonnes incorrect.`,
  csvUnknownKind: (detail: string) => `CSV, ligne ${detail} : type inconnu (login ou note attendu).`,
  csvInvalidUtf8: (detail: string) => `CSV, ligne ${detail} : texte qui n'est pas en UTF-8.`,
  nothingToBackUp: () => "Aucun coffre à sauvegarder.",
  emptyBackup: () => "Cette sauvegarde ne contient aucun coffre.",
  notInBackup: (detail: string) => `Cette sauvegarde ne contient pas de coffre « ${detail} ».`,
  // Le détail technique (taille, format) n'aide pas l'utilisateur : la CLI seule le donne.
  invalidBackup: () => "Sauvegarde inutilisable.",
  invalidSettings: () => "Réglages hors des valeurs permises.",
  ioError: (detail: string) => `Erreur de fichier : ${detail}`,
  fileUnwritable: (detail: string) => `Écriture du fichier impossible : ${detail}`,
  notConnected: () => "Cet appareil n'est connecté à aucun serveur.",
  alreadyConnected: () => "Cet appareil est déjà connecté à un serveur.",
  invalidConnectionKey: () => "Clé de connexion mal saisie : 13 groupes de 4 caractères séparés par des tirets.",
  serverError: (detail: string) => `Échange avec le serveur impossible : ${detail}.`,
  serverUnreachable: () => "Serveur injoignable.",
  notPublished: (detail: string) => `Le coffre « ${detail} » n'est pas publié sur le serveur de cet appareil.`,
  unpublishedChanges: (detail: string) => `« ${detail} » a des modifications non publiées : synchronisez-le d'abord.`,
  emptyPassword: () => "Le mot de passe est obligatoire.",
  emptyNote: () => "La note est obligatoire.",
  unexpected: (detail: string) => `Erreur : ${detail}`,
} satisfies Record<string, Message>;

export const fr = {
  appTitle: "PSWD Master",
  appVersion: (version: string) => `v${version}`,
  vaults: "Coffres de mot de passe",
  noVault: "Aucun coffre sur cet appareil.",
  loading: "Chargement…",
  cancel: "Annuler",
  newVault: "Nouveau coffre",
  vaultName: "Nom du coffre",
  vaultNameHint: "32 caractères au plus ; espaces et accents permis.",
  masterPassword: "Mot de passe maître",
  confirmation: "Confirmation",
  sharedVault: "Coffre commun",
  sharedVaultHint:
    "À partager avec d'autres comptes du serveur, qui l'ouvriront avec ce mot de passe maître. Ce choix est définitif.",
  create: "Créer",
  creating: "Création…",
  recoveryTitle: "Clé de récupération",
  recoveryExplanation:
    "Notez cette clé et rangez-la en lieu sûr, à l'écart du mot de passe : elle ne sera plus affichée. Elle permet de choisir un nouveau mot de passe maître en cas d'oubli.",
  recoveryNoted: "J'ai noté la clé et je l'ai rangée en lieu sûr.",
  continue: "Continuer",
  unlockTitle: (name: string) => `Déverrouiller « ${name} »`,
  unlock: "Déverrouiller",
  unlocking: "Déverrouillage…",
  entryCount: (count: number) => (count <= 1 ? `${count} entrée` : `${count} entrées`),
  lock: "Verrouiller",
  search: "Rechercher",
  searchHint: "Titre, identifiant, URL ou tag (Ctrl+F)",
  allTags: "Tous les tags",
  tagFilter: "Filtrer par tag",
  noEntry: "Aucune entrée.",
  pagination: "Pages des entrées",
  previousPage: "Page précédente",
  nextPage: "Page suivante",
  pageOf: (page: number, count: number) => `Page ${page} / ${count}`,
  chooseEntry: "Choisissez une entrée dans la liste.",
  back: "Retour",
  kinds: { login: "Mot de passe", note: "Note sécurisée" },
  username: "Identifiant",
  password: "Mot de passe",
  urls: "Adresses Web",
  openUrl: (url: string) => `Ouvrir ${url} dans le navigateur`,
  notes: "Notes",
  tags: "Tags",
  mask: "••••••••",
  show: "Afficher",
  showPassword: "Afficher le mot de passe",
  hide: "Masquer",
  copy: "Copier",
  copied: (seconds: number) => `Copié, effacé dans ${seconds} s.`,
  versions: (count: number) =>
    count === 0 ? "Aucune version précédente" : count === 1 ? "1 version précédente" : `${count} versions précédentes`,
  updated: (millis: number) =>
    `Modifiée le ${new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "short" }).format(millis)}`,
  newEntry: "Nouvelle entrée",
  editEntry: (title: string) => `Modifier « ${title} »`,
  edit: "Modifier",
  delete: "Supprimer",
  confirmDelete: (title: string) => `Supprimer « ${title} » ? Elle restera visible dans les sauvegardes antérieures.`,
  save: "Enregistrer",
  entrySaved: "Enregistrement réussi",
  saving: "Enregistrement…",
  kind: "Type",
  title: "Titre",
  passwordModes: {
    keep: "Inchangé",
    set: "Saisir",
    generate: "Générer",
  },
  length: "Longueur",
  symbols: "Avec des symboles",
  urlsHint: "Une adresse par ligne.",
  tagsHint: "Entrée, espace ou virgule ajoute le tag saisi ; ou choisissez un tag existant.",
  chosenTags: "Tags de l'entrée",
  newTag: "Nouveau tag",
  addTag: "Ajouter",
  removeTag: (tag: string) => `Retirer le tag ${tag}`,
  existingTags: "Tags existants",
  addExistingTag: (tag: string) => `Ajouter le tag ${tag}`,
  history: "Historique",
  passwordChanged: "mot de passe différent",
  manage: "Gérer le coffre",
  changePassword: "Changer le mot de passe maître",
  currentPassword: "Mot de passe maître actuel",
  newPassword: "Nouveau mot de passe maître",
  masterPasswordChanged: "Mot de passe maître changé.",
  forgotPassword: "Mot de passe oublié ?",
  recoverTitle: (name: string) => `Récupérer « ${name} »`,
  recoveryKey: "Clé de récupération",
  recover: "Enregistrer",
  importCsv: "Importer un CSV",
  importExplanation:
    "Le fichier est lu sans passer par l'interface. Tout le fichier est vérifié avant d'ajouter quoi que ce soit.",
  chooseFile: "Choisir le fichier",
  imported: (count: number) =>
    `${count <= 1 ? `${count} entrée importée` : `${count} entrées importées`}. Supprimez maintenant le fichier CSV : il contient vos mots de passe en clair.`,
  deleteVault: "Supprimer ce coffre",
  deleteForever: "Supprimer",
  deleteVaultExplanation: (name: string) =>
    `« ${name} » et ses entrées seront supprimés de cet appareil, sans retour possible : sauvegardez-le avant si besoin. Pour confirmer, retapez son nom.`,
  vaultDeleted: (name: string) => `Coffre « ${name} » supprimé.`,
  backup: "Sauvegarder",
  backupTitle: "Sauvegarder des coffres",
  backupExplanation:
    "Les coffres cochés sont réunis, toujours chiffrés, dans un seul fichier .pswd. Chacun se rouvrira avec son mot de passe maître actuel.",
  vaultsToBackUp: "Coffres à sauvegarder",
  backupSelection: (count: number) =>
    count <= 1 ? "Sauvegarder le coffre" : `Sauvegarder les ${count} coffres`,
  backingUp: "Sauvegarde…",
  backupFileTitle: "Enregistrer la sauvegarde des coffres",
  backedUp: (count: number) => (count <= 1 ? `${count} coffre sauvegardé.` : `${count} coffres sauvegardés.`),
  restore: "Restaurer",
  restoreTitle: "Restaurer une sauvegarde",
  restoreFileTitle: "Choisir un fichier de sauvegarde .pswd",
  chooseBackupFile: "fichier .pswd",
  restoreExplanation:
    "Le coffre est ajouté comme nouveau coffre : un coffre existant n'est jamais remplacé. Il s'ouvre avec le mot de passe maître en vigueur au moment de la sauvegarde.",
  restored: (name: string, target: string) =>
    name === target
      ? `Coffre « ${name} » restauré.`
      : `Coffre « ${name} » restauré sous le nom « ${target} » : ce nom était déjà pris.`,
  restoredMany: (restored: { name: string; target: string }[]) => {
    const renamed = restored
      .filter(({ name, target }) => name !== target)
      .map(({ name, target }) => `« ${name} » → « ${target} »`);
    return `${restored.length} coffres restaurés${renamed.length > 0 ? ` (renommés : ${renamed.join(", ")})` : ""}.`;
  },
  vaultsToRestore: "Coffres à restaurer",
  selectAll: "Tout sélectionner",
  restoreSelection: (count: number) =>
    count <= 1 ? "Restaurer le coffre sélectionné" : `Restaurer les ${count} coffres sélectionnés`,
  restoring: "Restauration…",
  settings: "Réglages",
  theme: "Thème",
  themes: { auto: "Comme le système", light: "Clair", dark: "Sombre" },
  settingsSaved: "Réglages enregistrés.",
  lockedAutomatically: "Coffre verrouillé automatiquement.",
  server: "Serveur",
  serverTitle: "Serveur maison",
  serverExplanation:
    "Connectez cet appareil à votre serveur pour retrouver vos coffres sur vos autres appareils. Le serveur ne reçoit que des coffres chiffrés.",
  serverAddress: "Adresse du serveur",
  serverAddressHint: "Par exemple 192.168.1.10 ; le port 8443 est ajouté s'il manque.",
  connectionKey: "Clé de connexion",
  connectionKeyHint: "13 groupes de 4 caractères séparés par des tirets.",
  deviceName: "Nom de cet appareil",
  deviceNameHint: "Par exemple « PC du salon » : il le distingue de vos autres appareils.",
  accountName: "Nom du compte",
  accountNameHint: "Seulement avec la clé administrateur affichée à l'installation du serveur ; sinon, laissez vide.",
  connect: "Se connecter",
  connecting: "Connexion…",
  connected: "Appareil connecté au serveur.",
  connectedTo: (address: string) => `Connecté au serveur ${address}.`,
  registeredServer: (address: string) => `Serveur enregistré : ${address}.`,
  serverTrouble: (since: number, reason: string) =>
    `Échanges impossibles depuis ${time.format(since)} : ${reason}.`,
  synchronize: "Synchroniser",
  synchronizing: "Synchronisation…",
  exchangeIncomplete: (message: string) =>
    `Échange avec le serveur incomplet. ${message} Réessayez avec « Synchroniser ».`,
  nothingToExchange: "Coffres à jour.",
  checkAddress: "Vérifiez l'adresse saisie.",
  exchangeSummary: (counts: { downloaded: number; published: number; sent: number; received: number }) => {
    const parts = [
      [counts.downloaded, "téléchargé", "téléchargés"],
      [counts.published, "publié", "publiés"],
      [counts.sent, "envoyé au serveur", "envoyés au serveur"],
      [counts.received, "mis à jour depuis le serveur", "mis à jour depuis le serveur"],
    ] as const;
    const shown = parts
      .filter(([count]) => count > 0)
      .map(([count, one, many], index) => {
        const noun = index > 0 ? "" : count === 1 ? "coffre " : "coffres ";
        return `${count} ${noun}${count === 1 ? one : many}`;
      });
    return `${shown.join(", ")}.`;
  },
  renamedOnDownload: (name: string, localName: string) => `« ${name} » enregistré sous « ${localName} » (nom déjà pris).`,
  vaultArrived: (name: string) => `Nouveau coffre « ${name} » reçu.`,
  gaveWay: (name: string, newName: string) => `« ${name} » renommé « ${newName} » : nom déjà pris sur le serveur.`,
  downloaded: (name: string, localName: string) =>
    name === localName ? `Coffre « ${name} » téléchargé.` : `« ${name} » téléchargé sous « ${localName} » (nom déjà pris).`,
  notDownloaded: (name: string, reason: string) => `Coffre « ${name} » du serveur non téléchargé : ${reason}.`,
  published: (name: string) => `Coffre « ${name} » publié.`,
  notPublished: (name: string, reason: string) => `Coffre « ${name} » non publié : ${reason}.`,
  sent: (name: string) => `Coffre « ${name} » envoyé au serveur.`,
  received: (name: string) => `Coffre « ${name} » mis à jour depuis le serveur.`,
  needsUnlock: (name: string) => `« ${name} » modifié ici et sur le serveur : déverrouillez-le.`,
  notUpdated: (name: string, reason: string) => `Coffre « ${name} » non synchronisé : ${reason}.`,
  leftElsewhere: (name: string) => `« ${name} » retiré : vous n'êtes plus membre.`,
  notLeft: (name: string, reason: string) =>
    `« ${name} » : vous n'êtes plus membre, mais sa suppression ici a échoué : ${reason}.`,
  deletedElsewhere: (name: string) => `« ${name} » supprimé depuis un autre appareil.`,
  notDeleted: (name: string, reason: string) =>
    `« ${name} » supprimé ailleurs, mais sa suppression ici a échoué : ${reason}.`,
  addDevice: "Ajouter un appareil",
  deviceKeyTitle: "Clé pour un nouvel appareil",
  deviceKeyExplanation: (address: string) =>
    `Sur le nouvel appareil, connectez-vous au serveur ${address} avec cette clé. Elle ne sert qu'une fois et expire dans 7 jours.`,
  close: "Fermer",
  disconnect: "Se déconnecter",
  disconnectTitle: "Se déconnecter du serveur",
  disconnectExplanation:
    "Cet appareil ne sera plus synchronisé. Vous pourrez vous reconnecter plus tard avec une nouvelle clé de connexion.",
  keepLocalCopies: "Garder une copie des coffres sur cet appareil",
  removeExplanation: "Les coffres à jour sur le serveur seront retirés ; ceux qui ont des modifications non publiées, gardés.",
  disconnecting: "Déconnexion…",
  disconnected: (removed: number, kept: number) => {
    const vaults = (count: number, one: string, many: string) =>
      `${count} ${count === 1 ? `coffre ${one}` : `coffres ${many}`}`;
    if (removed === 0 && kept === 0) {
      return "Appareil déconnecté.";
    }
    if (kept === 0) {
      return `Appareil déconnecté : ${vaults(removed, "retiré", "retirés")}.`;
    }
    if (removed === 0) {
      return `Appareil déconnecté : ${vaults(kept, "gardé", "gardés")}.`;
    }
    return `Appareil déconnecté : ${vaults(removed, "retiré", "retirés")}, ${kept} ${kept === 1 ? "gardé" : "gardés"}.`;
  },
  tokenNotRevoked: (reason: string) => `Serveur non prévenu (${reason}) : révoquez cet appareil dans l'administration.`,
  removedFromDevice: (name: string) => `« ${name} » retiré de cet appareil.`,
  keptOnDevice: (name: string, reason: string) => `« ${name} » gardé : ${reason}.`,
  vaultStates: {
    local: "Pas sur ce serveur : publié au prochain échange",
    synced: "Synchronisé",
    outdated: "Différent du serveur : mis à jour au prochain échange",
    needsUnlock: "Modifié ici et sur le serveur : à déverrouiller pour finir la synchro",
    removed: "Absent de votre compte sur le serveur : gardé sur cet appareil",
    unreachable: "Échanges avec le serveur impossibles : synchro en pause",
  },
  removedNotice:
    "Ce coffre n'est pas (ou plus) sur votre compte du serveur : il n'existe que sur cet appareil. Déverrouillez-le puis appuyez sur « Synchroniser » pour le publier, ou supprimez-le.",
  shared: "commun",
  invitations: "Invitations",
  invitation: (vaultName: string, invitedBy: string) => `${invitedBy} vous invite dans le coffre « ${vaultName} ».`,
  accept: "Accepter",
  decline: "Refuser",
  invitationAccepted: (invitedBy: string) => `Invitation acceptée : demandez le mot de passe à ${invitedBy}.`,
  invitationDeclined: "Invitation refusée.",
  members: "Membres",
  membersTitle: "Membres du coffre",
  pendingInvitations: "Invitations en attente",
  cancelInvitation: (name: string) => `Annuler l'invitation de ${name}`,
  invitationCancelled: "Invitation annulée.",
  inviteAccount: "Inviter un compte",
  invite: "Inviter",
  inviting: "Invitation…",
  noOneToInvite: "Tous les comptes du serveur sont déjà membres ou invités.",
  invited: "Invitation envoyée.",
  leaveVault: "Quitter ce coffre",
  leave: "Quitter",
  leaveExplanation: (name: string) => `« ${name} » sera retiré de vos appareils ; les autres membres le gardent.`,
  vaultLeft: (name: string) => `Vous avez quitté « ${name} ».`,
  lastMemberExplanation: (name: string) =>
    `Vous êtes le dernier membre : « ${name} » sera supprimé partout, sans retour possible : sauvegardez-le avant si besoin. Pour confirmer, retapez son nom.`,
  deleteWhere: "Que faire de ce coffre ?",
  deleteEverywhere: "Supprimer partout",
  deleteEverywhereExplanation:
    "Effacé du serveur et de vos appareils ; un appareil aux modifications non publiées garde sa copie.",
  forgetVault: "Retirer de cet appareil",
  forgetVaultExplanation: "Reste sur le serveur et vos autres appareils ; l'écran « Serveur » permet de le retélécharger.",
  forget: "Retirer",
  confirmVaultName: "Pour confirmer, retapez le nom du coffre.",
  ignoredVaults: "Coffres retirés de cet appareil",
  downloadAgain: "Télécharger",
  downloadAgainLabel: (name: string) => `Télécharger de nouveau « ${name} »`,
  account: (name: string, isAdmin: boolean) => `Compte : ${name}${isAdmin ? " (administrateur)" : ""}.`,
  administration: "Administration",
  adminTitle: "Administration du serveur",
  adminTabs: { accounts: "Comptes", vaults: "Coffres", keys: "Clés", server: "Serveur" },
  administrator: "administrateur",
  devices: "Appareils",
  noDevice: "Aucun appareil connecté.",
  searchAccount: "Rechercher un compte",
  noAccountFound: "Aucun compte ne porte ce nom.",
  accountPages: "Pages des comptes",
  deviceAdded: (name: string, millis: number) => `${name}, ajouté le ${day.format(millis)}`,
  revoke: "Révoquer",
  revokeDevice: (name: string) => `Révoquer ${name}`,
  deviceRevoked: (name: string) => `« ${name} » révoqué.`,
  makeAdmin: "Nommer administrateur",
  removeAdmin: "Retirer le rôle d'administrateur",
  lastAdministrator: "Dernier administrateur",
  roleGiven: (name: string) => `${name} est maintenant administrateur.`,
  roleRemoved: (name: string) => `${name} n'est plus administrateur.`,
  deleteAccount: "Supprimer le compte",
  deleteAccountExplanation: (name: string) =>
    `Le compte « ${name} », ses appareils et ses clés seront supprimés. Ses coffres restent sur le serveur ; ceux dont il était le seul membre n'en auront plus. Pour confirmer, retapez son nom.`,
  accountNameConfirmation: "Nom du compte",
  accountDeleted: (name: string) => `Compte « ${name} » supprimé.`,
  deleteOwnAccount: "Supprimer mon compte",
  deleteOwnAccountExplanation:
    "Votre compte et vos appareils seront supprimés du serveur, et cet appareil déconnecté ; vos coffres restent ici. Pour confirmer, retapez votre nom.",
  ownAccountDeleted: "Compte supprimé : appareil déconnecté.",
  addMember: "Ajouter un membre",
  memberName: "Nom du nouveau membre",
  createKey: "Créer sa clé",
  creatingKey: "Création…",
  memberKeyTitle: (name: string) => `Clé de connexion de ${name}`,
  memberKeyExplanation: (address: string) =>
    `Transmettez à ce membre l'adresse du serveur (${address}) et cette clé. Elle ne sert qu'une fois et expire dans 7 jours.`,
  noPendingKey: "Aucune clé de connexion en attente.",
  keyPurpose: (purpose: KeyPurpose, name: string) =>
    purpose === "admin"
      ? "Clé administrateur"
      : purpose === "member"
        ? `Nouveau membre : ${name}`
        : `Appareil de plus pour ${name}`,
  expires: (millis: number) => `expire le ${day.format(millis)}`,
  cancelKey: "Annuler la clé",
  keyCancelled: "Clé annulée.",
  noServerVault: "Aucun coffre sur le serveur.",
  searchServerVault: "Rechercher un coffre",
  noServerVaultFound: "Aucun coffre ne porte ce nom.",
  vaultPages: "Pages des coffres",
  privateVault: "privé",
  neverPublished: "jamais publié",
  version: (version: number) => `version ${version}`,
  noMember: "Aucun membre.",
  manageVault: (name: string) => `Gérer le coffre ${name}`,
  allVaults: "Tous les coffres",
  remove: "Retirer",
  removeMember: (name: string) => `Retirer ${name}`,
  memberRemoved: (name: string, vault: string) => `${name} retiré de « ${vault} ».`,
  attachExplanation: "Ce coffre n'a plus de membre : rattachez-y un compte pour qu'il le retrouve.",
  attachAccount: "Compte à rattacher",
  attach: "Rattacher",
  attached: (name: string, vault: string) => `${name} ajouté à « ${vault} ».`,
  versionsTitle: "Versions conservées",
  noVersion: "Aucune version publiée.",
  versionLine: (version: number, millis: number, size: number) =>
    `Version ${version} — ${dateTime.format(millis)} — ${Math.ceil(size / 1024)} Kio`,
  restoreVersion: (version: number) => `Restaurer la version ${version}`,
  currentVersion: "(actuelle)",
  versionPages: "Pages des versions",
  versionRestored: (name: string) => `Version restaurée dans « ${name} ».`,
  deleteServerVault: "Supprimer du serveur",
  deleteServerVaultExplanation: (name: string) =>
    `Le contenu du coffre « ${name} » et ses versions seront effacés du serveur. Les copies des appareils sont gardées : un membre pourra republier la sienne. Pour confirmer, retapez son nom.`,
  serverVaultDeleted: (name: string) => `Coffre « ${name} » supprimé du serveur.`,
  publicAddress: "Adresse publique",
  publicAddressHint:
    "Nom DNS ou adresse IP pour joindre le serveur hors de chez vous, par exemple maison.duckdns.org. Laissez vide pour l'effacer.",
  publicAddressSaved: "Adresse publique enregistrée.",
  serverBackup: "Sauvegarder le serveur",
  serverBackupExplanation:
    "Copie complète du serveur : comptes, coffres chiffrés et clé privée de son certificat. Rangez-la en lieu sûr, hors de cette machine.",
  serverBackupFileTitle: "Enregistrer la sauvegarde du serveur",
  serverBackedUp: "Sauvegarde du serveur enregistrée.",
  syncDone: "Coffre à jour.",
  syncNotConnected: "Cet appareil n'est connecté à aucun serveur.",
  syncRemoved: "Absent de votre compte sur le serveur : « Synchroniser » le publie.",
  syncLeft: "Vous n'êtes plus membre : coffre retiré de cet appareil.",
  syncDeleted: "Coffre supprimé depuis un autre appareil.",
  syncNameTaken: "Nom déjà pris sur le serveur : ce coffre sera renommé au retour à la liste.",
  passwordChangedElsewhere: "Mot de passe maître changé sur un autre appareil.",
  syncOffline: (detail: string) => `Synchro impossible : ${detail}. Les modifications seront publiées plus tard.`,
  syncConflict: "Le coffre a changé sur le serveur à chaque essai : synchronisez de nouveau.",
  syncRejected: (detail: string) => `Copie du serveur refusée (${detail}) : elle n'est pas remplacée.`,
  syncFailed: (detail: string) => `Synchro impossible : ${detail}.`,
  passwordRule: "Au moins 12 caractères, dont au moins 3 types parmi : minuscules, majuscules, chiffres, symboles.",
  validation: {
    nameRequired: "Le nom du coffre est obligatoire.",
    nameInvalid: "Nom invalide : 32 caractères au plus, sans tabulation ni retour à la ligne.",
    passwordRequired: "Le mot de passe maître est obligatoire.",
    passwordTooShort: "Le mot de passe maître doit contenir au moins 12 caractères.",
    passwordTooSimple:
      "Le mot de passe maître doit mélanger au moins 3 types de caractères parmi : minuscules, majuscules, chiffres, symboles.",
    passwordMismatch: "Les deux saisies ne correspondent pas.",
    titleRequired: "Le titre est obligatoire.",
    entryPasswordRequired: "Le mot de passe est obligatoire.",
    noteRequired: "La note est obligatoire.",
    lengthInvalid: "La longueur doit être un nombre entier entre 8 et 24.",
    addressRequired: "L'adresse du serveur est obligatoire.",
    keyRequired: "La clé de connexion est obligatoire.",
    deviceNameRequired: "Le nom de cet appareil est obligatoire.",
    memberNameRequired: "Le nom du nouveau membre est obligatoire.",
  },
  errors,
};

/** Toute langue doit fournir exactement ces textes. */
export type Texts = typeof fr;
