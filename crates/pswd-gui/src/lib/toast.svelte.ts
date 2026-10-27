/** Durée d'affichage par défaut d'une bulle de confirmation (ms). */
export const TOAST_DURATION = 3_000;

/** Bulle affichée en bas de l'écran ; `id` change à chaque message pour rejouer l'animation. */
export const toast = $state<{ id: number; message: string | null }>({ id: 0, message: null });

let timer: ReturnType<typeof setTimeout> | undefined;

/** Affiche une confirmation, qui disparaît d'elle-même. */
export function showToast(message: string, duration: number = TOAST_DURATION): void {
  clearTimeout(timer);
  toast.id += 1;
  toast.message = message;
  timer = setTimeout(hideToast, duration);
}

export function hideToast(): void {
  clearTimeout(timer);
  toast.message = null;
}
