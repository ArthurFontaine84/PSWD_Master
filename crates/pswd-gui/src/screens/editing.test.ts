import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  addEntry,
  deleteEntry,
  entryDetail,
  entryHistory,
  listEntries,
  listTags,
  updateEntry,
  type EntryDetail,
  type EntrySummary,
} from "../lib/api";
import { goBack } from "../lib/back";
import OpenVault from "./OpenVault.svelte";

vi.mock("../lib/api", () => ({
  listEntries: vi.fn(),
  listTags: vi.fn(),
  entryDetail: vi.fn(),
  revealPassword: vi.fn(),
  copyPassword: vi.fn(),
  lockVault: vi.fn(),
  addEntry: vi.fn(),
  updateEntry: vi.fn(),
  deleteEntry: vi.fn(),
  entryHistory: vi.fn(),
}));

const github: EntrySummary = {
  id: "1",
  kind: "login",
  title: "GitHub",
  username: "arthur",
  tags: ["dev"],
  hasPassword: true,
};
const githubDetail: EntryDetail = {
  ...github,
  urls: ["https://github.com"],
  notes: null,
  createdAt: 0,
  updatedAt: 0,
  versions: 2,
};

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listEntries).mockResolvedValue([github]);
  vi.mocked(listTags).mockResolvedValue([]);
  vi.mocked(entryDetail).mockResolvedValue(githubDetail);
});

function show() {
  const onExpired = vi.fn();
  render(OpenVault, {
    vault: { name: "perso", entries: 1 },
    onLocked: vi.fn(),
    onExpired,
    onDeleted: vi.fn(),
  });
  return { onExpired };
}

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

async function type(label: string, value: string) {
  await fireEvent.input(field(label), { target: { value } });
}

function expectVerbatim(element: HTMLElement) {
  expect(element).toHaveAttribute("autocapitalize", "off");
  expect(element).toHaveAttribute("autocorrect", "off");
  expect(element).toHaveAttribute("spellcheck", "false");
}

async function startNewEntry() {
  show();
  await screen.findByText("1 entrée");
  await fireEvent.click(screen.getByRole("button", { name: "Nouvelle entrée" }));
}

async function openGithub() {
  show();
  await fireEvent.click(await screen.findByRole("button", { name: /^GitHub/ }));
  await screen.findByRole("heading", { level: 3, name: "GitHub" });
}

describe("the back button inside a vault", () => {
  it("closes the form, then the entry, one at a time", async () => {
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Modifier" }));
    goBack();
    expect(await screen.findByRole("heading", { level: 3, name: "GitHub" })).toBeInTheDocument();
    goBack();
    expect(await screen.findByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });
});

describe("adding an entry", () => {
  it("saves a login with a typed password, then shows it", async () => {
    vi.mocked(addEntry).mockResolvedValue({ ...githubDetail, id: "9", title: "Box" });
    await startNewEntry();
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("Nouvelle entrée");
    await type("Titre", "Box");
    await type("Identifiant", "admin");
    await type("Mot de passe", "box-1234");
    await type("Adresses", "http://192.168.1.1");
    await type("Tags", "maison réseau");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(entryDetail).toHaveBeenCalledWith("9");
    });
    expect(addEntry).toHaveBeenCalledWith({
      kind: "login",
      title: "Box",
      username: "admin",
      password: { mode: "set", value: "box-1234" },
      urls: ["http://192.168.1.1"],
      notes: "",
      tags: ["maison", "réseau"],
    });
    expect(listEntries).toHaveBeenCalledTimes(2);
    expect(listTags).toHaveBeenCalledTimes(2);
  });

  it("keeps identifiers, addresses, tags and the search as typed on a phone keyboard", async () => {
    await startNewEntry();
    expectVerbatim(field("Identifiant"));
    expectVerbatim(field("Adresses"));
    expectVerbatim(field("Tags"));
    expectVerbatim(screen.getByRole("searchbox"));
    expect(field("Titre")).not.toHaveAttribute("autocapitalize");
  });

  it("lets the backend generate the password", async () => {
    vi.mocked(addEntry).mockResolvedValue(githubDetail);
    await startNewEntry();
    await type("Titre", "Banque");
    await fireEvent.click(screen.getByRole("radio", { name: "Générer" }));
    expect(screen.queryByLabelText("Mot de passe")).not.toBeInTheDocument();
    await type("Longueur", "30");
    await fireEvent.click(screen.getByRole("checkbox", { name: "Avec des symboles" }));
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(addEntry).toHaveBeenCalled();
    });
    expect(vi.mocked(addEntry).mock.calls[0]?.[0].password).toEqual({
      mode: "generate",
      length: 30,
      symbols: false,
    });
  });

  it("asks only the fields of a note", async () => {
    vi.mocked(addEntry).mockResolvedValue(githubDetail);
    await startNewEntry();
    await fireEvent.change(screen.getByRole("combobox", { name: "Type" }), {
      target: { value: "note" },
    });
    expect(screen.queryByLabelText("Identifiant")).not.toBeInTheDocument();
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    await type("Titre", "Wi-Fi");
    await type("Notes", "Box : ABCD");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(addEntry).toHaveBeenCalledWith(
        expect.objectContaining({ kind: "note", title: "Wi-Fi", notes: "Box : ABCD" }),
      );
    });
  });

  it("checks the form before sending anything", async () => {
    await startNewEntry();
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Le titre est obligatoire.");
    await type("Titre", "Banque");
    await fireEvent.click(screen.getByRole("radio", { name: "Générer" }));
    await type("Longueur", "4");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "La longueur doit être un nombre entier entre 8 et 128.",
    );
    expect(addEntry).not.toHaveBeenCalled();
  });

  it("reports a refusal and clears the typed password", async () => {
    vi.mocked(addEntry).mockRejectedValue({ code: "saveFailed", detail: "disque plein" });
    await startNewEntry();
    await type("Titre", "Box");
    await type("Mot de passe", "box-1234");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Enregistrement impossible : disque plein",
    );
    expect(field("Mot de passe").value).toBe("");
  });

  it("can be cancelled, and closes when another entry is chosen", async () => {
    await startNewEntry();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Nouvelle entrée" }));
    await fireEvent.click(screen.getByRole("button", { name: /^GitHub/ }));
    expect(await screen.findByRole("heading", { level: 3, name: "GitHub" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Enregistrer" })).not.toBeInTheDocument();
  });
});

describe("the vault locking itself during a save", () => {
  it("is reported to the parent", async () => {
    vi.mocked(addEntry).mockRejectedValue({ code: "vaultLocked", detail: null });
    const { onExpired } = show();
    await screen.findByText("1 entrée");
    await fireEvent.click(screen.getByRole("button", { name: "Nouvelle entrée" }));
    await type("Titre", "Box");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(onExpired).toHaveBeenCalled();
    });
  });
});

describe("editing an entry", () => {
  it("starts from its values and keeps its password by default", async () => {
    vi.mocked(updateEntry).mockResolvedValue({ ...githubDetail, title: "GitHub pro" });
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Modifier" }));
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("Modifier « GitHub »");
    expect(field("Titre").value).toBe("GitHub");
    expect(screen.queryByRole("combobox", { name: "Type" })).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Inchangé" })).toBeChecked();
    await type("Titre", "GitHub pro");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(updateEntry).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({ title: "GitHub pro", password: { mode: "keep" } }),
      );
    });
    await vi.waitFor(() => {
      expect(entryDetail).toHaveBeenCalledTimes(2);
    });
  });

  it("can remove the password", async () => {
    vi.mocked(updateEntry).mockResolvedValue(githubDetail);
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Modifier" }));
    await fireEvent.click(screen.getByRole("radio", { name: "Supprimer" }));
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(updateEntry).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({ password: { mode: "clear" } }),
      );
    });
  });
});

describe("deleting an entry", () => {
  it("asks for a confirmation first", async () => {
    vi.mocked(deleteEntry).mockResolvedValue();
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer" }));
    const confirmation = screen.getByRole("group", { name: "Supprimer" });
    expect(confirmation).toHaveTextContent("Supprimer « GitHub » ?");
    await fireEvent.click(within(confirmation).getByRole("button", { name: "Annuler" }));
    expect(deleteEntry).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer" }));
    vi.mocked(listEntries).mockResolvedValue([]);
    await fireEvent.click(
      within(screen.getByRole("group", { name: "Supprimer" })).getByRole("button", { name: "Supprimer" }),
    );
    expect(await screen.findByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
    expect(deleteEntry).toHaveBeenCalledWith("1");
    expect(await screen.findByText("Aucune entrée.")).toBeInTheDocument();
  });

  it("reports a failure", async () => {
    vi.mocked(deleteEntry).mockRejectedValue({ code: "saveFailed", detail: "disque plein" });
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer" }));
    await fireEvent.click(
      within(screen.getByRole("group", { name: "Supprimer" })).getByRole("button", { name: "Supprimer" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Enregistrement impossible : disque plein",
    );
  });
});

describe("history", () => {
  it("lists the previous versions without their passwords", async () => {
    vi.mocked(entryHistory).mockResolvedValue([
      { updatedAt: Date.UTC(2026, 9, 2), title: "GitHub", username: "arthur", passwordChanged: true },
      { updatedAt: Date.UTC(2026, 9, 1), title: "Git", username: null, passwordChanged: false },
    ]);
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    const versions = within(await screen.findByRole("list", { name: "Historique" })).getAllByRole(
      "listitem",
    );
    expect(versions).toHaveLength(2);
    expect(versions[0]).toHaveTextContent("GitHub — arthur (mot de passe différent)");
    expect(versions[1]).toHaveTextContent("Git");
    expect(versions[1]).not.toHaveTextContent("mot de passe différent");
    expect(entryHistory).toHaveBeenCalledWith("1");
    await fireEvent.click(screen.getByRole("button", { name: "Masquer l'historique" }));
    expect(screen.queryByRole("list", { name: "Historique" })).not.toBeInTheDocument();
  });

  it("is offered only when there are previous versions", async () => {
    vi.mocked(entryDetail).mockResolvedValue({ ...githubDetail, versions: 0 });
    await openGithub();
    expect(screen.queryByRole("button", { name: "Historique" })).not.toBeInTheDocument();
  });

  it("reports a failure", async () => {
    vi.mocked(entryHistory).mockRejectedValue({ code: "entryNotFound", detail: "1" });
    await openGithub();
    await fireEvent.click(screen.getByRole("button", { name: "Historique" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cette entrée n'existe plus.");
  });
});
