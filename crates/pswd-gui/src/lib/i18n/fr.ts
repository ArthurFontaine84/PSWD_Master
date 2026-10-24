export const fr = {
  appTitle: "PSWD Master",
  vaults: "Coffres",
  noVault: "Aucun coffre sur cet appareil.",
  loading: "Chargement…",
  errors: {
    configInvalid: (detail: string) => `Configuration invalide : ${detail}`,
    configUnreadable: (detail: string) => `Configuration illisible : ${detail}`,
    unexpected: (detail: string) => `Erreur : ${detail}`,
  },
};

/** Toute langue doit fournir exactement ces textes. */
export type Texts = typeof fr;
