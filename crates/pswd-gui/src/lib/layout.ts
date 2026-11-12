/** Écran large : la liste des entrées et leur détail côte à côte (même seuil que le CSS). */
export const WIDE_QUERY = "(min-width: 48rem)";

/** Dimensions d'une liste paginée, en rem. */
export interface ListLayout {
  /** Hauteur occupée hors de la liste : en-têtes, recherche, pagination, boutons. */
  reserved: number;
  /** Hauteur d'une ligne de la liste. */
  row: number;
  /** Lignes montrées même si la fenêtre est trop basse. */
  minRows: number;
  /** Éléments par ligne (cartes côte à côte sur un écran large). */
  columns: number;
}

/** Entrées d'un coffre : en-tête du coffre, recherche, compteur, pagination ; entrée avec
 * identifiant et tags. */
export const ENTRY_LIST: ListLayout = { reserved: 21.25, row: 5.75, minRows: 3, columns: 1 };

/** Partie commune du panneau d'administration : en-tête de l'appli, « Retour », titre, onglets. */
const ADMIN_PANEL = 22;

/** Comptes : recherche, pagination et ajout d'un membre ; carte avec un appareil, deux par ligne. */
export const ACCOUNT_LIST: ListLayout = { reserved: ADMIN_PANEL + 18, row: 11.5, minRows: 2, columns: 2 };

/** Coffres : recherche et pagination ; carte de trois lignes, deux par ligne. */
export const VAULT_LIST: ListLayout = { reserved: ADMIN_PANEL + 8, row: 6.25, minRows: 2, columns: 2 };

/** Clés en attente : pagination ; clé sur deux lignes et son bouton. */
export const KEY_LIST: ListLayout = { reserved: ADMIN_PANEL + 4, row: 4, minRows: 3, columns: 1 };

/** Versions d'un coffre : nom, deux membres, titres, pagination et boutons du coffre. */
export const VERSION_LIST: ListLayout = { reserved: ADMIN_PANEL + 20, row: 3.25, minRows: 3, columns: 1 };

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

/** Nombre d'éléments par page pour qu'une page tienne dans la fenêtre (hauteur en rem) sans défiler. */
export function pageSizeFor(windowHeight: number, list: ListLayout): number {
  const rows = Math.floor((windowHeight - list.reserved) / list.row);
  return Math.max(list.minRows, rows) * list.columns;
}

export function pageCount(total: number, size: number): number {
  return Math.max(1, Math.ceil(total / size));
}

/** Les éléments d'une page, numérotée à partir de 0. */
export function pageOf<T>(items: T[], page: number, size: number): T[] {
  return items.slice(page * size, (page + 1) * size);
}
