import { open, save } from "@tauri-apps/plugin-dialog";
import { describe, expect, it, vi } from "vitest";
import {
  pickBackupDestination,
  pickBackupFile,
  pickCsvDestination,
  pickCsvFile,
  pickServerBackupDestination,
} from "./files";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

describe("files", () => {
  it("asks the system for a CSV file", async () => {
    vi.mocked(open).mockResolvedValueOnce(null);
    await expect(pickCsvFile("Importer")).resolves.toBeNull();
    expect(vi.mocked(open).mock.calls).toEqual([
      [{ title: "Importer", multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] }],
    ]);
  });

  it("asks the system where to write a CSV file", async () => {
    vi.mocked(save).mockResolvedValueOnce("C:/modele.csv");
    await expect(pickCsvDestination("Enregistrer", "modele.csv")).resolves.toBe("C:/modele.csv");
    expect(save).toHaveBeenLastCalledWith({
      title: "Enregistrer",
      defaultPath: "modele.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
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

  it("asks the system where to write the server backup", async () => {
    vi.mocked(save).mockResolvedValueOnce("E:/serveur.db");
    await expect(pickServerBackupDestination("Sauvegarder", "serveur.db")).resolves.toBe("E:/serveur.db");
    expect(save).toHaveBeenLastCalledWith({
      title: "Sauvegarder",
      defaultPath: "serveur.db",
      filters: [{ name: "SQLite", extensions: ["db"] }],
    });
  });
});
