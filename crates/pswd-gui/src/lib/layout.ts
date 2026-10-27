/** Écran large : la liste des entrées et leur détail côte à côte (même seuil que le CSS). */
export const WIDE_QUERY = "(min-width: 48rem)";

/** Hauteur occupée hors de la liste : en-tête du coffre, recherche, compteur, pagination (rem). */
const RESERVED_HEIGHT = 21.25;
/** Hauteur d'une entrée avec identifiant et tags (rem). */
const ENTRY_HEIGHT = 5.75;
const MIN_PAGE_SIZE = 3;

/** Suit la largeur de l'écran ; renvoie la fonction qui arrête le suivi. */
export function watchWide(
  onChange: (wide: boolean) => void,
  view: Partial<Pick<Window, "matchMedia">> = window,
): () => void {
  if (typeof view.matchMedia !== "function") {
    return () => undefined;
  }
  const query = view.matchMedia(WIDE_QUERY);
  const listener = (event: MediaQueryListEvent) => {
    onChange(event.matches);
  };
  onChange(query.matches);
  query.addEventListener("change", listener);
  return () => {
    query.removeEventListener("change", listener);
  };
}

/** Taille d'un `rem` en pixels (la taille calculée est toujours en px) : elle grandit sur les très
 * grands écrans (styles.css). */
export function remSize(root: HTMLElement = document.documentElement): number {
  return Number.parseFloat(getComputedStyle(root).fontSize);
}

/** Nombre d'entrées par page pour qu'une page tienne dans la fenêtre (hauteur en rem) sans défiler. */
export function pageSizeFor(windowHeight: number): number {
  return Math.max(MIN_PAGE_SIZE, Math.floor((windowHeight - RESERVED_HEIGHT) / ENTRY_HEIGHT));
}

export function pageCount(total: number, size: number): number {
  return Math.max(1, Math.ceil(total / size));
}

/** Les éléments d'une page, numérotée à partir de 0. */
export function pageOf<T>(items: T[], page: number, size: number): T[] {
  return items.slice(page * size, (page + 1) * size);
}
