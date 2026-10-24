/** Message construit à partir du détail d'une erreur (qui peut être ignoré). */
type Message = (detail: string) => string;

const errors = {
  configInvalid: (detail: string) => `Configuration invalide : ${detail}`,
  configUnreadable: (detail: string) => `Configuration illisible : ${detail}`,
  unknownVault: (detail: string) => `Coffre inconnu : ${detail}.`,
  vaultExists: (detail: string) => `Un coffre nommé ${detail} existe déjà.`,
  invalidVaultName: (detail: string) =>
    `Nom de coffre invalide : « ${detail} » (lettres, chiffres, - et _ ; 64 caractères au plus).`,
  emptyPassword: () => "Le mot de passe maître ne peut pas être vide.",
  wrongPassword: () => "Mot de passe maître incorrect.",
  vaultFolderExists: () => "Un dossier de coffre existe déjà à cet emplacement.",
  vaultUnreadable: (detail: string) => `Coffre illisible : ${detail}`,
  unexpected: (detail: string) => `Erreur : ${detail}`,
} satisfies Record<string, Message>;

export const fr = {
  appTitle: "PSWD Master",
  vaults: "Coffres",
  noVault: "Aucun coffre sur cet appareil.",
  loading: "Chargement…",
  cancel: "Annuler",
  newVault: "Nouveau coffre",
  vaultName: "Nom du coffre",
  vaultNameHint: "Lettres, chiffres, - et _.",
  masterPassword: "Mot de passe maître",
  confirmation: "Confirmation",
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
  vaultTitle: (name: string) => `Coffre « ${name} »`,
  entryCount: (count: number) => (count <= 1 ? `${count} entrée` : `${count} entrées`),
  lock: "Verrouiller",
  lockedForInactivity: "Coffre verrouillé après une période d'inactivité.",
  validation: {
    nameRequired: "Le nom du coffre est obligatoire.",
    nameInvalid: "Nom invalide : lettres, chiffres, - et _ uniquement, 64 caractères au plus.",
    passwordRequired: "Le mot de passe maître est obligatoire.",
    passwordMismatch: "Les deux saisies ne correspondent pas.",
  },
  errors,
};

/** Toute langue doit fournir exactement ces textes. */
export type Texts = typeof fr;
