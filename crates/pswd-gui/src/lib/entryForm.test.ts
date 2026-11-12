import { describe, expect, it } from "vitest";
import type { EntryDetail } from "./api";
import { emptyForm, formFrom, formProblem, toInput } from "./entryForm";

const detail: EntryDetail = {
  id: "1",
  kind: "login",
  title: "GitHub",
  username: "arthur",
  tags: ["dev", "perso"],
  hasPassword: true,
  urls: ["https://github.com", "https://gist.github.com"],
  notes: "compte pro",
  createdAt: 0,
  updatedAt: 0,
  versions: 0,
};

describe("entry form", () => {
  it("starts empty for a new login with a typed password", () => {
    expect(emptyForm()).toEqual({
      kind: "login",
      title: "",
      username: "",
      passwordMode: "set",
      password: "",
      length: 24,
      symbols: true,
      urls: "",
      notes: "",
      tags: [],
      tagDraft: "",
    });
  });

  it("starts from the entry when editing, keeping its password", () => {
    expect(formFrom(detail)).toEqual({
      ...emptyForm(),
      title: "GitHub",
      username: "arthur",
      passwordMode: "keep",
      urls: "https://github.com\nhttps://gist.github.com",
      notes: "compte pro",
      tags: ["dev", "perso"],
    });
    const note = formFrom({ ...detail, kind: "note", username: null, notes: null, urls: [], tags: [] });
    expect(note).toMatchObject({ kind: "note", username: "", notes: "", urls: "", tags: [] });
  });

  it("requires a title, a password or a note, and a valid length to generate", () => {
    expect(formProblem({ ...emptyForm(), title: "  " })).toBe("Le titre est obligatoire.");
    expect(formProblem({ ...emptyForm(), title: "GitHub" })).toBe("Le mot de passe est obligatoire.");
    expect(formProblem({ ...emptyForm(), title: "GitHub", password: "x" })).toBeNull();
    expect(formProblem({ ...emptyForm(), title: "GitHub", passwordMode: "keep" })).toBeNull();
    const note = { ...emptyForm(), kind: "note" as const, title: "Wi-Fi" };
    expect(formProblem({ ...note, notes: "  " })).toBe("La note est obligatoire.");
    expect(formProblem({ ...note, notes: "Box : ABCD" })).toBeNull();
    const generating = { ...emptyForm(), title: "GitHub", passwordMode: "generate" as const };
    expect(formProblem(generating)).toBeNull();
    for (const length of [8, 24]) {
      expect(formProblem({ ...generating, length })).toBeNull();
    }
    for (const length of [7, 25, 12.5, Number.NaN]) {
      expect(formProblem({ ...generating, length })).toBe(
        "La longueur doit être un nombre entier entre 8 et 24.",
      );
    }
    expect(formProblem({ ...generating, kind: "note", notes: "texte", length: 3 })).toBeNull();
    expect(formProblem({ ...emptyForm(), title: "GitHub", password: "x", length: 3 })).toBeNull();
  });

  it("converts the fields for the backend", () => {
    const values = {
      ...emptyForm(),
      title: "GitHub",
      username: "arthur",
      password: "hunter2",
      urls: " https://github.com \n\n https://gist.github.com ",
      notes: "compte pro",
      tags: ["dev", "perso"],
      tagDraft: " Maison ",
    };
    expect(toInput(values)).toEqual({
      kind: "login",
      title: "GitHub",
      username: "arthur",
      password: { mode: "set", value: "hunter2" },
      urls: ["https://github.com", "https://gist.github.com"],
      notes: "compte pro",
      tags: ["dev", "perso", "maison"],
    });
    expect(toInput({ ...values, passwordMode: "generate", length: 20, symbols: false }).password).toEqual({
      mode: "generate",
      length: 20,
      symbols: false,
    });
    expect(toInput({ ...values, passwordMode: "keep" }).password).toEqual({ mode: "keep" });
    expect(toInput({ ...emptyForm(), title: "Wi-Fi" })).toMatchObject({ urls: [], tags: [] });
  });
});
