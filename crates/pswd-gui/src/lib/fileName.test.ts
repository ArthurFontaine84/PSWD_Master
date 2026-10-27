import { describe, expect, it } from "vitest";
import { displayedFileName } from "./fileName";

describe("file name", () => {
  it("keeps only the name of a backup file, when the address gives it", () => {
    expect(displayedFileName("D:/sauvegardes/s.pswd")).toBe("s.pswd");
    expect(displayedFileName("C:\\Users\\a\\s.pswd")).toBe("s.pswd");
    expect(displayedFileName("/home/a/100%/s.pswd")).toBe("s.pswd");
    const shared = "content://com.android.externalstorage.documents/document/primary%3ADownload%2Fs.pswd";
    expect(displayedFileName(shared)).toBe("s.pswd");
    const opaque = "content://com.android.providers.downloads.documents/document/msf%3A1000";
    expect(displayedFileName(opaque)).toBeNull();
  });
});
