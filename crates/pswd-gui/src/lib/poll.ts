import { EXCHANGE_INTERVAL } from "./sync";

/** Relance `task` au rythme de l'échange automatique, seulement quand l'appli est au premier
 * plan ; renvoie la fonction qui arrête. */
export function everyExchange(task: () => void): () => void {
  const timer = setInterval(() => {
    if (document.visibilityState === "visible") {
      task();
    }
  }, EXCHANGE_INTERVAL);
  return () => {
    clearInterval(timer);
  };
}
