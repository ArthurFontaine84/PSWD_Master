import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listVaults } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("api", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("asks the backend for the vault names", async () => {
    vi.mocked(invoke).mockResolvedValue(["perso", "travail"]);
    await expect(listVaults()).resolves.toEqual(["perso", "travail"]);
    expect(invoke).toHaveBeenCalledWith("list_vaults");
  });
});
