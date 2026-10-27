/**
 * Signale l'activité de l'utilisateur au plus une fois par `interval` (ms) : un clic ou une
 * touche repousse le verrouillage sans appeler le backend à chaque mouvement.
 */
export function throttle(send: () => void, interval: number, now: () => number = Date.now): () => void {
  let last = Number.NEGATIVE_INFINITY;
  return () => {
    const current = now();
    if (current - last >= interval) {
      last = current;
      send();
    }
  };
}
