import { describe, expect, it } from "vitest";
import { byName } from "./search";

describe("byName", () => {
  const accounts = [{ name: "Hélène" }, { name: "Camille" }, { name: "Arthur" }];

  it("keeps the names containing the text, whatever the accents and the case", () => {
    expect(byName(accounts, "HELE")).toEqual([{ name: "Hélène" }]);
    expect(byName(accounts, "ill")).toEqual([{ name: "Camille" }]);
    expect(byName([{ name: "ÉTÉ" }], "été")).toEqual([{ name: "ÉTÉ" }]);
  });

  it("keeps everything for an empty search, and nothing without a match", () => {
    expect(byName(accounts, "  ")).toEqual(accounts);
    expect(byName(accounts, "zoé")).toEqual([]);
  });
});
