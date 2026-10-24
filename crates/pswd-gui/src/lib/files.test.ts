import { open, save } from "@tauri-apps/plugin-dialog";
import { describe, expect, it, vi } from "vitest";
import { pickBackupDestination, pickBackupFile, pickCsvFile, pickFolder } from "./files";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

describe("files", () => {
  it("asks the system for a folder or a CSV file", async () => {
    vi.mocked(open).mockResolvedValueOnce("E:/").mockResolvedValueOnce(null);
    await expect(pickFolder("Choisir")).resolves.toBe("E:/");
    await expect(pickCsvFile("Importer")).resolves.toBeNull();
    expect(vi.mocked(open).mock.calls).toEqual([
      [{ title: "Choisir", directory: true, multiple: false }],
      [{ title: "Importer", multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] }],
    ]);
  });

  it("asks the system for a backup file to read or to write", async () => {
    const filters = [{ name: "Sauvegarde PSWD", extensions: ["pswd"] }];
    vi.mocked(open).mockResolvedValueOnce("E:/s.pswd");
    vi.mocked(save).mockResolvedValueOnce(null);
    await expect(pickBackupFile("Restaurer")).resolves.toBe("E:/s.pswd");
    await expect(pickBackupDestination("Enregistrer", "s.pswd")).resolves.toBeNull();
    expect(open).toHaveBeenLastCalledWith({ title: "Restaurer", multiple: false, filters });
    expect(save).toHaveBeenCalledWith({ title: "Enregistrer", defaultPath: "s.pswd", filters });
  });
});
