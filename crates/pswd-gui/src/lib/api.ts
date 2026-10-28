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

/** État d'un coffre vis-à-vis du serveur de l'appareil. */
export type VaultServerState = "toPublish" | "published" | "detached";

export interface ServerStatus {
  /** `null` si l'appareil n'est connecté à aucun serveur. */
  address: string | null;
  vaults: { name: string; state: VaultServerState }[];
}

/** Échec pour un coffre ; `reason` est un message du backend. */
export interface VaultFailure {
  name: string;
  reason: string;
}

/** Résultat d'un échange avec le serveur. */
export interface Exchange {
  /** `name` sur le serveur, `localName` sur cet appareil (différent si le nom était pris). */
  downloaded: { name: string; localName: string }[];
  notDownloaded: VaultFailure[];
  published: string[];
  notPublished: VaultFailure[];
}

export interface Disconnection {
  /** Le jeton n'a pas pu être révoqué. */
  revokeError: string | null;
  /** Coffres retirés de cet appareil ; ils restent sur le serveur. */
  removed: string[];
  /** `reason` à `null` : des modifications ne sont pas encore sur le serveur. */
  kept: { name: string; reason: string | null }[];
}

export type SyncState = "synced" | "notConnected" | "detached" | "offline" | "conflict" | "rejected" | "failed";

/** Résultat de la synchro du coffre ouvert. */
export interface SyncReport {
  vault: string;
  state: SyncState;
  detail: string | null;
  /** La synchro a changé le contenu du coffre : il est à relire. */
  changed: boolean;
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

/** Écrit les coffres `names` dans le fichier `.pswd` choisi ; renvoie leur nombre. */
export function backupToFile(path: string, names: string[]): Promise<number> {
  return invoke<number>("backup_to_file", { path, names });
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

/** Le serveur de l'appareil et l'état de chaque coffre, sans échange avec le serveur. */
export function serverStatus(): Promise<ServerStatus> {
  return invoke<ServerStatus>("server_status");
}

/** `accountName` reste vide, sauf avec une clé administrateur. */
export async function connectServer(
  address: string,
  key: string,
  deviceName: string,
  accountName: string,
): Promise<void> {
  await invoke("connect_server", { address, key, deviceName, accountName });
}

/** Télécharge les nouveaux coffres du compte et publie ceux de l'appareil. */
export function syncServer(): Promise<Exchange> {
  return invoke<Exchange>("sync_server");
}

/** Clé de connexion d'un autre appareil : seulement à la demande de l'utilisateur. */
export function addDeviceKey(): Promise<string> {
  return invoke<string>("add_device_key");
}

export function disconnectServer(keepLocal: boolean): Promise<Disconnection> {
  return invoke<Disconnection>("disconnect_server", { keepLocal });
}

/** Synchronise le coffre ouvert et renvoie le résultat. */
export function syncVault(): Promise<SyncReport> {
  return invoke<SyncReport>("sync_vault");
}

/** Appelle `handler` quand le coffre se verrouille par inactivité. */
export function onVaultLocked(handler: () => void): Promise<UnlistenFn> {
  return listen("vault-locked", handler);
}

/** Appelle `handler` à la fin de chaque synchro automatique du coffre ouvert. */
export function onVaultSynced(handler: (report: SyncReport) => void): Promise<UnlistenFn> {
  return listen<SyncReport>("vault-synced", (event) => {
    handler(event.payload);
  });
}
