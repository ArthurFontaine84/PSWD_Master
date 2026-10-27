// Fenêtres système de choix d'un fichier : seul le chemin choisi est transmis, le backend lit et
// écrit lui-même.
import { open, save } from "@tauri-apps/plugin-dialog";

const BACKUP_FILTER = { name: "Sauvegarde PSWD", extensions: ["pswd"] };

/** Un fichier de sauvegarde `.pswd` à lire, ou `null` si l'utilisateur a annulé. */
export function pickBackupFile(title: string): Promise<string | null> {
  return open({ title, multiple: false, filters: [BACKUP_FILTER] });
}

/** L'emplacement où enregistrer une sauvegarde `.pswd`, ou `null` si l'utilisateur a annulé. */
export function pickBackupDestination(title: string, defaultPath: string): Promise<string | null> {
  return save({ title, defaultPath, filters: [BACKUP_FILTER] });
}

/** Un fichier CSV choisi par l'utilisateur, ou `null` s'il a annulé. */
export function pickCsvFile(title: string): Promise<string | null> {
  return open({ title, multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
}
