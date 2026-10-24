import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createVault, listVaults, lockVault, unlockVault } from "../lib/api";
import CreateVault from "./CreateVault.svelte";
import OpenVault from "./OpenVault.svelte";
import RecoveryKey from "./RecoveryKey.svelte";
import Unlock from "./Unlock.svelte";
import VaultList from "./VaultList.svelte";

vi.mock("../lib/api", () => ({
  listVaults: vi.fn(),
  createVault: vi.fn(),
  unlockVault: vi.fn(),
  lockVault: vi.fn(),
}));

const created = { vault: { name: "perso", entries: 0 }, recoveryKey: "AAAA-BBBB" };

beforeEach(() => {
  vi.resetAllMocks();
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

describe("VaultList", () => {
  it("opens a vault or starts a new one", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    const onOpen = vi.fn();
    const onCreate = vi.fn();
    render(VaultList, { onOpen, onCreate });
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole("button", { name: "travail" }));
    expect(onOpen).toHaveBeenCalledWith("travail");
    await fireEvent.click(screen.getByRole("button", { name: "Nouveau coffre" }));
    expect(onCreate).toHaveBeenCalled();
  });

  it("shows the notice, the empty list and the errors", async () => {
    vi.mocked(listVaults).mockResolvedValue([]);
    render(VaultList, { notice: "Coffre verrouillé.", onOpen: vi.fn(), onCreate: vi.fn() });
    expect(screen.getByRole("status")).toHaveTextContent("Coffre verrouillé.");
    expect(await screen.findByText("Aucun coffre sur cet appareil.")).toBeInTheDocument();
  });

  it("shows a translated error", async () => {
    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    render(VaultList, { onOpen: vi.fn(), onCreate: vi.fn() });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
  });
});

describe("CreateVault", () => {
  function fill(name: string, password: string, confirmation: string) {
    void fireEvent.input(field("Nom du coffre"), { target: { value: name } });
    void fireEvent.input(field("Mot de passe maître"), { target: { value: password } });
    void fireEvent.input(field("Confirmation"), { target: { value: confirmation } });
  }

  it("checks the form before sending anything", async () => {
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    fill("perso", "maître", "maitre");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    expect(createVault).not.toHaveBeenCalled();
  });

  it("creates the vault and clears the passwords", async () => {
    vi.mocked(createVault).mockResolvedValue(created);
    const onCreated = vi.fn();
    render(CreateVault, { onCreated, onCancel: vi.fn() });
    fill("perso", "maître", "maître");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    await vi.waitFor(() => {
      expect(onCreated).toHaveBeenCalledWith(created);
    });
    expect(createVault).toHaveBeenCalledWith("perso", "maître");
    expect(field("Mot de passe maître").value).toBe("");
    expect(field("Confirmation").value).toBe("");
  });

  it("shows the backend refusal", async () => {
    vi.mocked(createVault).mockRejectedValue({ code: "vaultExists", detail: "perso" });
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    fill("perso", "maître", "maître");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Un coffre nommé perso existe déjà.",
    );
    expect(screen.getByRole("button", { name: "Créer" })).toBeEnabled();
  });

  it("can be cancelled", async () => {
    const onCancel = vi.fn();
    render(CreateVault, { onCreated: vi.fn(), onCancel });
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onCancel).toHaveBeenCalled();
  });
});

describe("RecoveryKey", () => {
  it("continues only once the key is noted", async () => {
    const onDone = vi.fn();
    render(RecoveryKey, { recoveryKey: "AAAA-BBBB", onDone });
    expect(screen.getByText("AAAA-BBBB")).toBeInTheDocument();
    const next = screen.getByRole("button", { name: "Continuer" });
    expect(next).toBeDisabled();
    await fireEvent.click(screen.getByRole("checkbox"));
    await fireEvent.click(next);
    expect(onDone).toHaveBeenCalled();
  });
});

describe("Unlock", () => {
  it("unlocks with the password and clears it", async () => {
    vi.mocked(unlockVault).mockResolvedValue({ name: "perso", entries: 3 });
    const onUnlocked = vi.fn();
    render(Unlock, { name: "perso", onUnlocked, onCancel: vi.fn() });
    expect(screen.getByRole("heading")).toHaveTextContent("Déverrouiller « perso »");
    await fireEvent.input(field("Mot de passe maître"), { target: { value: "maître" } });
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 3 });
    });
    expect(unlockVault).toHaveBeenCalledWith("perso", "maître");
    expect(field("Mot de passe maître").value).toBe("");
  });

  it("explains a wrong password and can be cancelled", async () => {
    vi.mocked(unlockVault).mockRejectedValue({ code: "wrongPassword", detail: null });
    const onCancel = vi.fn();
    render(Unlock, { name: "perso", onUnlocked: vi.fn(), onCancel });
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Mot de passe maître incorrect.");
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onCancel).toHaveBeenCalled();
  });
});

describe("OpenVault", () => {
  it("shows the vault and locks it", async () => {
    vi.mocked(lockVault).mockResolvedValue();
    const onLocked = vi.fn();
    render(OpenVault, { vault: { name: "perso", entries: 2 }, onLocked });
    expect(screen.getByRole("heading")).toHaveTextContent("Coffre « perso »");
    expect(screen.getByText("2 entrées")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Verrouiller" }));
    await vi.waitFor(() => {
      expect(onLocked).toHaveBeenCalled();
    });
    expect(lockVault).toHaveBeenCalled();
  });

  it("counts a single entry in the singular", () => {
    render(OpenVault, { vault: { name: "perso", entries: 1 }, onLocked: vi.fn() });
    expect(screen.getByText("1 entrée")).toBeInTheDocument();
  });
});
