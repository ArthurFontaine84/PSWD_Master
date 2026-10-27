import type { Theme } from "./api";

/** Applique le thème : `auto` laisse le système décider (clair ou sombre). */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement): void {
  root.dataset.theme = theme;
}
