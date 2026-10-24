import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createVault,
  currentVault,
  listVaults,
  lockVault,
  onVaultLocked,
  unlockVault,
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
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["list_vaults"],
      ["create_vault", { name: "perso", password: "maître" }],
      ["unlock_vault", { name: "perso", password: "maître" }],
      ["lock_vault"],
      ["current_vault"],
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
