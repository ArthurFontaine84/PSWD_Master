// Seul point de contact avec le backend Rust : chaque commande Tauri a sa fonction typée.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Erreur renvoyée par une commande : un code stable et un détail éventuel. */
export interface UiError {
  code: string;
  detail: string | null;
}

export type DeviceKind = "desktop" | "mobile";

/** Coffre ouvert, sans aucun secret. */
export interface VaultInfo {
  name: string;
  entries: number;
}

export type EntryKind = "login" | "note";

/** Une ligne de la liste : jamais de mot de passe. */
export interface EntrySummary {
  id: string;
  kind: EntryKind;
  title: string;
  username: string | null;
  tags: string[];
  hasPassword: boolean;
}

/** Le détail d'une entrée, toujours sans son mot de passe. */
export interface EntryDetail extends EntrySummary {
  urls: string[];
  notes: string | null;
  /** Millisecondes depuis l'époque Unix. */
  createdAt: number;
  updatedAt: number;
  versions: number;
}

/** Ce que devient le mot de passe ; `generate` le fait tirer par le backend. */
export type PasswordChoice =
  | { mode: "keep" }
  | { mode: "clear" }
  | { mode: "set"; value: string }
  | { mode: "generate"; length: number; symbols: boolean };

export interface EntryInput {
  kind: EntryKind;
  title: string;
  username: string;
  password: PasswordChoice;
  urls: string[];
  notes: string;
  tags: string[];
}

/** Une version précédente, sans mot de passe. */
export interface HistoryItem {
  updatedAt: number;
  title: string;
  username: string | null;
  passwordChanged: boolean;
}

export interface TagCount {
  tag: string;
  count: number;
}

export type Theme = "auto" | "light" | "dark";

export interface Settings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  backgroundLockSeconds: number;
  theme: Theme;
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

/** L'application passe en arrière-plan : sur mobile, le verrouillage approche. */
export async function appHidden(): Promise<void> {
  await invoke("app_hidden");
}

/** L'application revient au premier plan : le coffre encore ouvert, ou `null`. */
export function appShown(): Promise<VaultInfo | null> {
  return invoke<VaultInfo | null>("app_shown");
}

/** Le type d'appareil : sur mobile, une sauvegarde est un fichier, jamais un dossier. */
export function deviceKind(): Promise<DeviceKind> {
  return invoke<DeviceKind>("device_kind");
}

export function listEntries(text: string, tag: string | null): Promise<EntrySummary[]> {
  return invoke<EntrySummary[]>("list_entries", { text, tag });
}

export function entryDetail(id: string): Promise<EntryDetail> {
  return invoke<EntryDetail>("entry_detail", { id });
}

export function listTags(): Promise<TagCount[]> {
  return invoke<TagCount[]>("list_tags");
}

/** Le mot de passe en clair : seulement quand l'utilisateur demande à le voir. */
export function revealPassword(id: string): Promise<string> {
  return invoke<string>("reveal_password", { id });
}

/** Copie faite par le backend ; renvoie le délai d'effacement, en secondes. */
export function copyPassword(id: string): Promise<number> {
  return invoke<number>("copy_password", { id });
}

export function addEntry(entry: EntryInput): Promise<EntryDetail> {
  return invoke<EntryDetail>("add_entry", { entry });
}

export function updateEntry(id: string, entry: EntryInput): Promise<EntryDetail> {
  return invoke<EntryDetail>("update_entry", { id, entry });
}

export async function deleteEntry(id: string): Promise<void> {
  await invoke("delete_entry", { id });
}

export function entryHistory(id: string): Promise<HistoryItem[]> {
  return invoke<HistoryItem[]>("entry_history", { id });
}

/** Le mot de passe actuel est redemandé par le backend. */
export async function changePassword(current: string, password: string): Promise<void> {
  await invoke("change_password", { current, password });
}

export function recoverVault(name: string, recoveryKey: string, password: string): Promise<VaultInfo> {
  return invoke<VaultInfo>("recover_vault", { name, recoveryKey, password });
}

export async function deleteVault(name: string, confirmation: string): Promise<void> {
  await invoke("delete_vault", { name, confirmation });
}

/** Le backend lit le fichier : son contenu en clair ne passe pas par l'interface. */
export function importEntries(path: string): Promise<number> {
  return invoke<number>("import_entries", { path });
}

/** Nom proposé pour une sauvegarde en un seul fichier, daté par le backend. */
export function backupFileName(): Promise<string> {
  return invoke<string>("backup_file_name");
}

/** Écrit tous les coffres dans le fichier `.pswd` choisi ; renvoie leur nombre. */
export function backupToFile(path: string): Promise<number> {
  return invoke<number>("backup_to_file", { path });
}

export function backupFileContents(path: string): Promise<string[]> {
  return invoke<string[]>("backup_file_contents", { path });
}

/** Restaure un coffre d'un fichier `.pswd` ; renvoie le nom retenu. */
export function restoreFromFile(path: string, name: string): Promise<string> {
  return invoke<string>("restore_from_file", { path, name });
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("save_settings", { settings });
}

/** Appelle `handler` quand le coffre se verrouille par inactivité. */
export function onVaultLocked(handler: () => void): Promise<UnlistenFn> {
  return listen("vault-locked", handler);
}
