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
  onVaultSynced,
  revealPassword,
  unlockVault,
  updateEntry,
  serverStatus,
  connectServer,
  syncServer,
  refreshServer,
  addDeviceKey,
  disconnectServer,
  syncVault,
  checkVault,
  listAccounts,
  vaultMembers,
  inviteMember,
  listInvitations,
  acceptInvitation,
  deleteInvitation,
  leaveVault,
  accountStatus,
  deleteVaultEverywhere,
  adminAccounts,
  addMember,
  deleteAccount,
  setAdmin,
  revokeDevice,
  pendingKeys,
  cancelKey,
  serverVaults,
  deleteServerVault,
  attachAccount,
  removeMember,
  vaultVersions,
  restoreVersion,
  setPublicAddress,
  serverBackupFileName,
  backupServerToFile,
  type EntryInput,
  type SyncReport,
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
    await serverStatus();
    await connectServer("pswd.local", "AAAA", "PC", "Arthur");
    await syncServer();
    await refreshServer();
    await addDeviceKey();
    await disconnectServer(false);
    await listAccounts();
    await vaultMembers("famille");
    await inviteMember("famille", "a1");
    await listInvitations();
    await acceptInvitation("i1");
    await deleteInvitation("i1");
    await leaveVault("famille");
    await syncVault();
    await checkVault();
    await accountStatus();
    await deleteVaultEverywhere("perso", "perso");
    await adminAccounts();
    await addMember("Camille");
    await deleteAccount("a2", "Camille", "Camille");
    await setAdmin("a2", true);
    await revokeDevice("d1");
    await pendingKeys();
    await cancelKey("k1");
    await serverVaults();
    await deleteServerVault("v1", "famille", "famille");
    await attachAccount("v1", "a2");
    await removeMember("v1", "a2");
    await vaultVersions("v1");
    await restoreVersion("v1", 3);
    await setPublicAddress("maison");
    await serverBackupFileName();
    await backupServerToFile("E:/serveur.db");
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["list_vaults"],
      ["create_vault", { name: "perso", password: "maître", shared: false }],
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
      ["server_status"],
      ["connect_server", { address: "pswd.local", key: "AAAA", deviceName: "PC", accountName: "Arthur" }],
      ["sync_server"],
      ["refresh_server"],
      ["add_device_key"],
      ["disconnect_server", { keepLocal: false }],
      ["list_accounts"],
      ["vault_members", { vault: "famille" }],
      ["invite_member", { vault: "famille", account: "a1" }],
      ["list_invitations"],
      ["accept_invitation", { id: "i1" }],
      ["delete_invitation", { id: "i1" }],
      ["leave_vault", { vault: "famille" }],
      ["sync_vault"],
      ["check_vault"],
      ["account_status"],
      ["delete_vault_everywhere", { name: "perso", confirmation: "perso" }],
      ["admin_accounts"],
      ["add_member", { name: "Camille" }],
      ["delete_account", { id: "a2", name: "Camille", confirmation: "Camille" }],
      ["set_admin", { id: "a2", isAdmin: true }],
      ["revoke_device", { id: "d1" }],
      ["pending_keys"],
      ["cancel_key", { id: "k1" }],
      ["server_vaults"],
      ["delete_server_vault", { id: "v1", name: "famille", confirmation: "famille" }],
      ["attach_account", { vault: "v1", account: "a2" }],
      ["remove_member", { vault: "v1", account: "a2" }],
      ["vault_versions", { vault: "v1" }],
      ["restore_version", { vault: "v1", version: 3 }],
      ["set_public_address", { address: "maison" }],
      ["server_backup_file_name"],
      ["backup_server_to_file", { path: "E:/serveur.db" }],
    ]);
  });

  it("passes on the result of each automatic sync", async () => {
    const stop = vi.fn();
    vi.mocked(listen).mockClear().mockResolvedValue(stop);
    const handler = vi.fn();
    await expect(onVaultSynced(handler)).resolves.toBe(stop);
    const report: SyncReport = { vault: "perso", state: "synced", detail: null, changed: true };
    const call = vi.mocked(listen).mock.lastCall;
    expect(call?.[0]).toBe("vault-synced");
    call?.[1]({ event: "vault-synced", id: 1, payload: report });
    expect(handler).toHaveBeenCalledWith(report);
  });

  it("listens to the automatic lock", async () => {
    const stop = vi.fn();
    vi.mocked(listen).mockResolvedValue(stop);
    const handler = vi.fn();
    await expect(onVaultLocked(handler)).resolves.toBe(stop);
    expect(listen).toHaveBeenCalledWith("vault-locked", handler);
  });
});
