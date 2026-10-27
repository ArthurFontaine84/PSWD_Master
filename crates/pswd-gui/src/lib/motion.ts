/** Durée d'une animation (ms) : nulle si le système demande de réduire les animations. */
export function motion(duration: number, view: Partial<Pick<Window, "matchMedia">> = window): number {
  if (typeof view.matchMedia !== "function") {
    return 0;
  }
  return view.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : duration;
}
