// Seul point de contact avec le backend Rust : chaque commande Tauri a sa fonction typée.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Erreur renvoyée par une commande : un code stable et un détail éventuel. */
export interface UiError {
  code: string;
  detail: string | null;
}

/** Coffre ouvert, sans aucun secret. */
export interface VaultInfo {
  name: string;
  entries: number;
}

export interface CreatedVault {
  vault: VaultInfo;
  /** À afficher une seule fois, puis à oublier. */
  recoveryKey: string;
}

export function listVaults(): Promise<string[]> {
  return invoke<string[]>("list_vaults");
}

export function createVault(name: string, password: string): Promise<CreatedVault> {
  return invoke<CreatedVault>("create_vault", { name, password });
}

export function unlockVault(name: string, password: string): Promise<VaultInfo> {
  return invoke<VaultInfo>("unlock_vault", { name, password });
}

export async function lockVault(): Promise<void> {
  await invoke("lock_vault");
}

/** Le coffre ouvert, ou `null` ; chaque appel repousse le verrouillage automatique. */
export function currentVault(): Promise<VaultInfo | null> {
  return invoke<VaultInfo | null>("current_vault");
}

/** Appelle `handler` quand le coffre se verrouille par inactivité. */
export function onVaultLocked(handler: () => void): Promise<UnlistenFn> {
  return listen("vault-locked", handler);
}
