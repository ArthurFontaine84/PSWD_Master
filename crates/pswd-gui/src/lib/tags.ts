// Saisie des tags en étiquettes. Rust normalise de toute façon (`pswd_core::Tags`) ; l'interface
// applique la même règle pour montrer ce qui sera enregistré.

/** Séparateurs de tags dans le texte saisi. */
const SEPARATORS = /[\s,]+/;

/** Nombre de tags existants proposés à la fois. */
const MAX_SUGGESTIONS = 8;

/** Les tags de `text` ajoutés à `tags` : en minuscules, sans vide ni doublon. */
export function addTags(tags: string[], text: string): string[] {
  const added = [...tags];
  for (const word of text.split(SEPARATORS)) {
    const tag = word.toLowerCase();
    if (tag !== "" && !added.includes(tag)) {
      added.push(tag);
    }
  }
  return added;
}

/**
 * Saisie en cours : les mots terminés par un séparateur (espace, virgule) deviennent des tags, le
 * dernier mot reste à compléter. Un clavier de téléphone n'envoie pas toujours la touche tapée :
 * c'est le texte qui est examiné.
 */
export function splitDraft(tags: string[], draft: string): { tags: string[]; draft: string } {
  const lastWord = draft.search(/[^\s,]*$/);
  return { tags: addTags(tags, draft.slice(0, lastWord)), draft: draft.slice(lastWord) };
}

/** Tags existants à proposer : pas encore choisis, et contenant le début de saisie. */
export function suggestTags(known: string[], chosen: string[], draft: string): string[] {
  const wanted = draft.trim().toLowerCase();
  return known.filter((tag) => !chosen.includes(tag) && tag.includes(wanted)).slice(0, MAX_SUGGESTIONS);
}
