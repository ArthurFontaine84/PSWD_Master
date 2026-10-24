import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  addEntry,
  backupContents,
  backupVaults,
  changePassword,
  copyPassword,
  createVault,
  currentVault,
  deleteEntry,
  deleteVault,
  getSettings,
  importEntries,
  recoverVault,
  restoreVault,
  saveSettings,
  entryDetail,
  entryHistory,
  listEntries,
  listTags,
  listVaults,
  lockVault,
  onVaultLocked,
  revealPassword,
  unlockVault,
  updateEntry,
  type EntryInput,
} from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("api", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("calls each backend command with its arguments", async () => {
    vi.mocked(invoke).mockResolvedValue("réponse");
    await listVaults();
    await createVault("perso", "maître");
    await unlockVault("perso", "maître");
    await lockVault();
    await expect(currentVault()).resolves.toBe("réponse");
    await listEntries("git", "dev");
    await entryDetail("1");
    await listTags();
    await revealPassword("1");
    await copyPassword("1");
    const entry: EntryInput = {
      kind: "note",
      title: "Wi-Fi",
      username: "",
      password: { mode: "keep" },
      urls: [],
      notes: "",
      tags: [],
    };
    await addEntry(entry);
    await updateEntry("1", entry);
    await deleteEntry("1");
    await entryHistory("1");
    await changePassword("maître", "nouveau");
    await recoverVault("perso", "AAAA", "nouveau");
    await deleteVault("perso", "perso");
    await importEntries("C:/import.csv");
    await backupVaults("E:/");
    await backupContents("E:/pswd-sauvegardes/1");
    await restoreVault("E:/pswd-sauvegardes/1", "perso");
    await getSettings();
    const settings = {
      autoLockMinutes: 5,
      clipboardClearSeconds: 20,
      backgroundLockSeconds: 60,
      theme: "dark" as const,
    };
    await saveSettings(settings);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["list_vaults"],
      ["create_vault", { name: "perso", password: "maître" }],
      ["unlock_vault", { name: "perso", password: "maître" }],
      ["lock_vault"],
      ["current_vault"],
      ["list_entries", { text: "git", tag: "dev" }],
      ["entry_detail", { id: "1" }],
      ["list_tags"],
      ["reveal_password", { id: "1" }],
      ["copy_password", { id: "1" }],
      ["add_entry", { entry }],
      ["update_entry", { id: "1", entry }],
      ["delete_entry", { id: "1" }],
      ["entry_history", { id: "1" }],
      ["change_password", { current: "maître", password: "nouveau" }],
      ["recover_vault", { name: "perso", recoveryKey: "AAAA", password: "nouveau" }],
      ["delete_vault", { name: "perso", confirmation: "perso" }],
      ["import_entries", { path: "C:/import.csv" }],
      ["backup_vaults", { destination: "E:/" }],
      ["backup_contents", { folder: "E:/pswd-sauvegardes/1" }],
      ["restore_vault", { folder: "E:/pswd-sauvegardes/1", name: "perso" }],
      ["get_settings"],
      ["save_settings", { settings }],
    ]);
  });

  it("listens to the automatic lock", async () => {
    const stop = vi.fn();
    vi.mocked(listen).mockResolvedValue(stop);
    const handler = vi.fn();
    await expect(onVaultLocked(handler)).resolves.toBe(stop);
    expect(listen).toHaveBeenCalledWith("vault-locked", handler);
  });
});
