// Seul point de contact avec le backend Rust : chaque commande Tauri a sa fonction typée.
import { invoke } from "@tauri-apps/api/core";

/** Erreur renvoyée par une commande : un code stable et un détail éventuel. */
export interface UiError {
  code: string;
  detail: string | null;
}

export function listVaults(): Promise<string[]> {
  return invoke<string[]>("list_vaults");
}
