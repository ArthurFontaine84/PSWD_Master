/**
 * Attributs d'un champ saisi tel quel (nom de coffre, identifiant, clé, adresses, tags) :
 * ni majuscule automatique, ni correction, ni suggestion du clavier.
 */
export const verbatim = {
  autocomplete: "off",
  autocapitalize: "off",
  autocorrect: "off",
  spellcheck: false,
} as const;
