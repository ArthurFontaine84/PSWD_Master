// Bouton retour d'Android : chaque écran ouvert s'inscrit, le dernier inscrit répond en premier.
import { exit, onBackButtonPress } from "@tauri-apps/api/app";

/** Remonte d'un écran ; `false` quand il n'y a rien à remonter à ce niveau. */
export type BackHandler = () => boolean;

const handlers: BackHandler[] = [];

/** Inscrit un écran ; renvoie de quoi le désinscrire (à sa destruction). */
export function onBack(handler: BackHandler): () => void {
  handlers.push(handler);
  return () => {
    const index = handlers.lastIndexOf(handler);
    if (index !== -1) {
      handlers.splice(index, 1);
    }
  };
}

/** Remonte d'un écran, ou ferme l'application depuis le premier. */
export function goBack(): void {
  for (const handler of [...handlers].reverse()) {
    if (handler()) {
      return;
    }
  }
  void exit();
}

/** Remplace le comportement par défaut du bouton retour ; renvoie de quoi le rétablir. */
export async function listenBackButton(): Promise<() => void> {
  const listener = await onBackButtonPress(goBack);
  return () => {
    void listener.unregister();
  };
}
