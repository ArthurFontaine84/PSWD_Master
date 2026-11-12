import type { DotColor } from "./vaultDot";

/** Durée d'affichage par défaut d'une bulle de confirmation (ms). */
export const TOAST_DURATION = 3_000;

/**
 * Bulle affichée en bas de l'écran ; `id` change à chaque message pour rejouer l'animation.
 * Sa couleur suit celle des pastilles : verte pour une confirmation.
 */
export const toast = $state<{ id: number; message: string | null; color: DotColor }>({
  id: 0,
  message: null,
  color: "green",
});

let timer: ReturnType<typeof setTimeout> | undefined;

export interface ToastOptions {
  color?: DotColor;
  duration?: number;
}

/** Affiche un message, qui disparaît de lui-même. */
export function showToast(message: string, { color = "green", duration = TOAST_DURATION }: ToastOptions = {}): void {
  clearTimeout(timer);
  toast.id += 1;
  toast.message = message;
  toast.color = color;
  timer = setTimeout(hideToast, duration);
}

export function hideToast(): void {
  clearTimeout(timer);
  toast.message = null;
}
