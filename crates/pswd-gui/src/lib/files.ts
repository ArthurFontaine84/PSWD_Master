// Fenêtres système de choix d'un fichier ou d'un dossier : seul le chemin choisi est transmis,
// le backend lit et écrit lui-même.
import { open } from "@tauri-apps/plugin-dialog";

/** Un dossier choisi par l'utilisateur, ou `null` s'il a annulé. */
export function pickFolder(title: string): Promise<string | null> {
  return open({ title, directory: true, multiple: false });
}

/** Un fichier CSV choisi par l'utilisateur, ou `null` s'il a annulé. */
export function pickCsvFile(title: string): Promise<string | null> {
  return open({ title, multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
}
