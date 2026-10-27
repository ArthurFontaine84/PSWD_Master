import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  addEntry,
  changePassword,
  copyPassword,
  createVault,
  currentVault,
  appHidden,
  appShown,
  deviceKind,
  deleteEntry,
  deleteVault,
  getSettings,
  importEntries,
  recoverVault,
  backupFileName,
  backupToFile,
  backupFileContents,
  restoreFromFile,
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
    await deviceKind();
    await appHidden();
    await expect(appShown()).resolves.toBe("réponse");
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
    await backupFileName();
    await backupToFile("E:/s.pswd", ["perso"]);
    await backupFileContents("E:/s.pswd");
    await restoreFromFile("E:/s.pswd", "perso");
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
      ["device_kind"],
      ["app_hidden"],
      ["app_shown"],
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
      ["backup_file_name"],
      ["backup_to_file", { path: "E:/s.pswd", names: ["perso"] }],
      ["backup_file_contents", { path: "E:/s.pswd" }],
      ["restore_from_file", { path: "E:/s.pswd", name: "perso" }],
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
