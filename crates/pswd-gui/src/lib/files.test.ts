import { open } from "@tauri-apps/plugin-dialog";
import { describe, expect, it, vi } from "vitest";
import { pickCsvFile, pickFolder } from "./files";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

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
});
