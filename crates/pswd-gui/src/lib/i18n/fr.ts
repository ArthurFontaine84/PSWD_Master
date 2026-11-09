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
    `Nom de coffre invalide : « ${detail} » (lettres, chiffres, - et _ ; 64 caractères au plus).`,
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
  invalidBackup: (detail: string) => `Sauvegarde inutilisable : ${detail}`,
  invalidSettings: () => "Réglages hors des valeurs permises.",
  ioError: (detail: string) => `Erreur de fichier : ${detail}`,
  fileUnwritable: (detail: string) => `Écriture du fichier impossible : ${detail}`,
  notConnected: () => "Cet appareil n'est connecté à aucun serveur.",
  alreadyConnected: () => "Cet appareil est déjà connecté à un serveur.",
  invalidConnectionKey: () => "Clé de connexion mal saisie : 13 groupes de 4 caractères séparés par des tirets.",
  serverError: (detail: string) => `Échange avec le serveur impossible : ${detail}.`,
  serverUnreachable: () => "Serveur injoignable.",
  notPublished: (detail: string) => `Le coffre « ${detail} » n'est pas publié sur le serveur de cet appareil.`,
  unpublishedChanges: (detail: string) =>
    `Le coffre « ${detail} » a des modifications que le serveur n'a pas encore : synchronisez-le d'abord.`,
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
  vaultNameHint: "Lettres, chiffres, - et _.",
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
    clear: "Supprimer",
  },
  length: "Longueur",
  symbols: "Avec des symboles",
  urlsHint: "Une adresse par ligne.",
  tagsHint: "Séparés par des espaces.",
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
    `Le coffre « ${name} » et toutes ses entrées seront supprimés de cet appareil, sans retour possible. Une sauvegarde permet de le restaurer plus tard. Pour confirmer, retapez son nom.`,
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
  renamedOnDownload: (name: string, localName: string) =>
    `Coffre « ${name} » enregistré sous le nom « ${localName} » : ce nom était déjà pris.`,
  vaultArrived: (name: string) => `Nouveau coffre « ${name} » reçu du serveur.`,
  newInvitation: (vaultName: string, invitedBy: string) =>
    `Nouvelle invitation : ${invitedBy} vous invite dans le coffre « ${vaultName} ».`,
  downloaded: (name: string, localName: string) =>
    name === localName
      ? `Coffre « ${name} » téléchargé.`
      : `Coffre « ${name} » téléchargé sous le nom « ${localName} » : ce nom était déjà pris.`,
  notDownloaded: (name: string, reason: string) => `Coffre « ${name} » du serveur non téléchargé : ${reason}.`,
  published: (name: string) => `Coffre « ${name} » publié.`,
  notPublished: (name: string, reason: string) => `Coffre « ${name} » non publié : ${reason}.`,
  sent: (name: string) => `Coffre « ${name} » envoyé au serveur.`,
  received: (name: string) => `Coffre « ${name} » mis à jour depuis le serveur.`,
  needsUnlock: (name: string) =>
    `Le coffre « ${name} » a été modifié ici et sur le serveur : déverrouillez-le pour terminer la synchro.`,
  notUpdated: (name: string, reason: string) => `Coffre « ${name} » non synchronisé : ${reason}.`,
  leftElsewhere: (name: string) =>
    `Coffre « ${name} » supprimé de cet appareil : vous n'êtes plus membre de ce coffre.`,
  notLeft: (name: string, reason: string) =>
    `Vous n'êtes plus membre du coffre « ${name} », mais il n'a pas pu être supprimé de cet appareil : ${reason}.`,
  deletedElsewhere: (name: string) =>
    `Coffre « ${name} » supprimé de cet appareil : il a été supprimé partout depuis un autre appareil.`,
  notDeleted: (name: string, reason: string) =>
    `Le coffre « ${name} » a été supprimé partout, mais n'a pas pu être supprimé de cet appareil : ${reason}.`,
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
  removeExplanation:
    "Seuls les coffres déjà à jour sur le serveur seront retirés de cet appareil ; ceux qui ont des modifications non publiées seront gardés.",
  disconnecting: "Déconnexion…",
  disconnected: "Appareil déconnecté du serveur.",
  tokenNotRevoked: (reason: string) =>
    `Le serveur n'a pas pu être prévenu (${reason}) : révoquez cet appareil depuis le panneau d'administration.`,
  removedFromDevice: (name: string) => `Coffre « ${name} » retiré de cet appareil ; il reste sur le serveur.`,
  keptOnDevice: (name: string, reason: string | null) =>
    `Coffre « ${name} » gardé : ${reason ?? "des modifications ne sont pas encore sur le serveur"}.`,
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
  absentFromAccount: (names: string[]) =>
    names.length === 1
      ? `Coffre « ${names.join("")} » absent de votre compte sur le serveur : gardé sur cet appareil. Pour le publier, ouvrez-le puis appuyez sur « Synchroniser ».`
      : `Coffres absents de votre compte sur le serveur, gardés sur cet appareil : ${names.map((name) => `« ${name} »`).join(", ")}. Pour les publier, ouvrez-les puis appuyez sur « Synchroniser ».`,
  otherAccountReminder:
    "Les coffres gardés sur cet appareil restent liés à ce compte : s'ils appartiennent à quelqu'un d'autre, supprimez-les avant de connecter un autre compte.",
  invitations: "Invitations",
  invitation: (vaultName: string, invitedBy: string) => `${invitedBy} vous invite dans le coffre « ${vaultName} ».`,
  accept: "Accepter",
  decline: "Refuser",
  invitationAccepted: (invitedBy: string) => `Demandez à ${invitedBy} le mot de passe maître du coffre pour l'ouvrir.`,
  invitationDeclined: (vaultName: string) => `Invitation au coffre « ${vaultName} » refusée.`,
  members: "Membres",
  membersTitle: "Membres du coffre",
  pendingInvitations: "Invitations en attente",
  cancelInvitation: (name: string) => `Annuler l'invitation de ${name}`,
  invitationCancelled: (name: string) => `Invitation de ${name} annulée.`,
  inviteAccount: "Inviter un compte",
  invite: "Inviter",
  inviting: "Invitation…",
  noOneToInvite: "Tous les comptes du serveur sont déjà membres ou invités.",
  invited: (name: string) =>
    `Invitation envoyée à ${name}. Transmettez-lui le mot de passe maître du coffre, sans passer par le serveur.`,
  leaveVault: "Quitter ce coffre",
  leave: "Quitter",
  leaveExplanation: (name: string) =>
    `Le coffre « ${name} » sera supprimé de cet appareil et de vos autres appareils ; les autres membres le gardent. Pour le rejoindre plus tard, il faudra une nouvelle invitation.`,
  vaultLeft: (name: string) => `Vous avez quitté le coffre « ${name} ».`,
  lastMemberExplanation: (name: string) =>
    `Vous êtes le dernier membre du coffre « ${name} » : il sera supprimé du serveur et de tous vos appareils, sans retour possible. Une sauvegarde permet de le restaurer plus tard. Pour confirmer, retapez son nom.`,
  deleteWhere: "Que faire de ce coffre ?",
  deleteEverywhere: "Supprimer partout",
  deleteEverywhereExplanation:
    "Le coffre et ses versions seront effacés du serveur, puis de vos autres appareils à leur prochain échange. Un appareil qui a des modifications pas encore publiées garde sa copie.",
  forgetVault: "Retirer de cet appareil",
  forgetVaultExplanation:
    "Le coffre reste sur le serveur et sur vos autres appareils ; il ne sera plus téléchargé ici. L'écran « Serveur » permet de le télécharger de nouveau.",
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
  deviceAdded: (name: string, millis: number) => `${name}, ajouté le ${day.format(millis)}`,
  revoke: (name: string) => `Révoquer ${name}`,
  deviceRevoked: (name: string) => `Appareil « ${name} » révoqué : il ne peut plus joindre le serveur.`,
  makeAdmin: "Nommer administrateur",
  removeAdmin: "Retirer le rôle d'administrateur",
  roleGiven: (name: string) => `${name} est maintenant administrateur.`,
  roleRemoved: (name: string) => `${name} n'est plus administrateur.`,
  deleteAccount: "Supprimer le compte",
  deleteAccountExplanation: (name: string) =>
    `Le compte « ${name} », ses appareils et ses clés seront supprimés. Ses coffres restent sur le serveur ; ceux dont il était le seul membre n'en auront plus. Pour confirmer, retapez son nom.`,
  accountNameConfirmation: "Nom du compte",
  accountDeleted: (name: string) => `Compte « ${name} » supprimé.`,
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
  keyCancelled: "Clé annulée : elle ne peut plus servir.",
  noServerVault: "Aucun coffre sur le serveur.",
  privateVault: "privé",
  neverPublished: "jamais publié",
  version: (version: number) => `version ${version}`,
  noMember: "Aucun membre.",
  manageVault: (name: string) => `Gérer le coffre ${name}`,
  allVaults: "Tous les coffres",
  removeMember: (name: string) => `Retirer ${name}`,
  memberRemoved: (name: string, vault: string) => `${name} a été retiré du coffre « ${vault} ».`,
  attachExplanation: "Ce coffre n'a plus de membre : rattachez-y un compte pour qu'il le retrouve.",
  attachAccount: "Compte à rattacher",
  attach: "Rattacher",
  attached: (name: string, vault: string) => `${name} est maintenant membre du coffre « ${vault} ».`,
  versionsTitle: "Versions conservées",
  noVersion: "Aucune version publiée.",
  versionLine: (version: number, millis: number, size: number) =>
    `Version ${version} — ${dateTime.format(millis)} — ${Math.ceil(size / 1024)} Kio`,
  restoreVersion: (version: number) => `Restaurer la version ${version}`,
  versionRestored: (name: string) =>
    `Version restaurée dans le nouveau coffre « ${name} » ; ses membres le recevront au prochain échange.`,
  deleteServerVault: "Supprimer du serveur",
  deleteServerVaultExplanation: (name: string) =>
    `Le contenu du coffre « ${name} » et ses versions seront effacés du serveur. Les copies des appareils sont gardées : un membre pourra republier la sienne. Pour confirmer, retapez son nom.`,
  serverVaultDeleted: (name: string) => `Coffre « ${name} » supprimé du serveur.`,
  publicAddress: "Adresse publique",
  publicAddressHint:
    "Nom DNS ou adresse IP pour joindre le serveur hors de chez vous, par exemple maison.duckdns.org. Laissez vide pour l'effacer.",
  publicAddressSaved: "Adresse publique enregistrée : les appareils la reçoivent au prochain échange.",
  serverBackup: "Sauvegarder le serveur",
  serverBackupExplanation:
    "Copie complète du serveur : comptes, coffres chiffrés et clé privée de son certificat. Rangez-la en lieu sûr, hors de cette machine.",
  serverBackupFileTitle: "Enregistrer la sauvegarde du serveur",
  serverBackedUp: "Sauvegarde du serveur enregistrée.",
  syncDone: "Coffre à jour.",
  syncNotConnected: "Cet appareil n'est connecté à aucun serveur.",
  syncRemoved:
    "Coffre absent de votre compte sur le serveur : il n'existe que sur cet appareil. Pour le publier, appuyez sur « Synchroniser ».",
  syncLeft: "Vous n'êtes plus membre de ce coffre : il a été supprimé de cet appareil.",
  syncDeleted: "Ce coffre a été supprimé partout depuis un autre appareil : il a été supprimé de cet appareil.",
  passwordChangedElsewhere: "Le mot de passe maître de ce coffre a été changé sur un autre appareil.",
  syncOffline: (detail: string) => `Synchro impossible : ${detail}. Les modifications seront publiées plus tard.`,
  syncConflict: "Le coffre a changé sur le serveur à chaque essai : synchronisez de nouveau.",
  syncRejected: (detail: string) => `Copie du serveur refusée (${detail}) : elle n'est pas remplacée.`,
  syncFailed: (detail: string) => `Synchro impossible : ${detail}.`,
  passwordRule: "Au moins 12 caractères, dont au moins 3 types parmi : minuscules, majuscules, chiffres, symboles.",
  validation: {
    nameRequired: "Le nom du coffre est obligatoire.",
    nameInvalid: "Nom invalide : lettres, chiffres, - et _ uniquement, 64 caractères au plus.",
    passwordRequired: "Le mot de passe maître est obligatoire.",
    passwordTooShort: "Le mot de passe maître doit contenir au moins 12 caractères.",
    passwordTooSimple:
      "Le mot de passe maître doit mélanger au moins 3 types de caractères parmi : minuscules, majuscules, chiffres, symboles.",
    passwordMismatch: "Les deux saisies ne correspondent pas.",
    titleRequired: "Le titre est obligatoire.",
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
