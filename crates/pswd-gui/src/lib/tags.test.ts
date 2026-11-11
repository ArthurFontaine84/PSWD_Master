import { describe, expect, it } from "vitest";
import { addTags, splitDraft, suggestTags } from "./tags";

describe("addTags", () => {
  it("adds each word in lower case, without empty tag nor duplicate", () => {
    expect(addTags(["dev"], " Perso,  pro DEV perso ")).toEqual(["dev", "perso", "pro"]);
    expect(addTags(["dev"], " , ")).toEqual(["dev"]);
  });

  it("leaves the original list untouched", () => {
    const tags = ["dev"];
    addTags(tags, "perso");
    expect(tags).toEqual(["dev"]);
  });
});

describe("splitDraft", () => {
  it("turns the finished words into tags and keeps the last one to complete", () => {
    expect(splitDraft(["dev"], "Maison, rés")).toEqual({ tags: ["dev", "maison"], draft: "rés" });
    expect(splitDraft([], "maison ")).toEqual({ tags: ["maison"], draft: "" });
    expect(splitDraft([], "mai")).toEqual({ tags: [], draft: "mai" });
  });
});

describe("suggestTags", () => {
  const known = ["dev", "maison", "perso", "pro"];

  it("proposes the existing tags not yet chosen that contain the typed text", () => {
    expect(suggestTags(known, ["perso"], "")).toEqual(["dev", "maison", "pro"]);
    expect(suggestTags(known, [], " P")).toEqual(["perso", "pro"]);
    expect(suggestTags(known, [], "zz")).toEqual([]);
  });

  it("proposes eight tags at most", () => {
    const many = Array.from({ length: 12 }, (_, index) => `tag${String(index)}`);
    expect(suggestTags(many, [], "tag")).toHaveLength(8);
  });
});
