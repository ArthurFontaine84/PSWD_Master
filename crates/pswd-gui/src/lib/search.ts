/** Forme comparable d'un texte : sans accents ni majuscules. */
function folded(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();
}

/** Les éléments dont le nom contient le texte cherché, accents et majuscules ignorés. */
export function byName<T extends { name: string }>(items: T[], query: string): T[] {
  const wanted = folded(query.trim());
  return items.filter(({ name }) => folded(name).includes(wanted));
}
