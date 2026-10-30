import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createVault,
  deviceKind,
  listInvitations,
  listVaults,
  serverStatus,
  syncServer,
  unlockVault,
} from "../lib/api";
import Toast from "../components/Toast.svelte";
import CreateVault from "./CreateVault.svelte";
import RecoveryKey from "./RecoveryKey.svelte";
import Unlock from "./Unlock.svelte";
import VaultList from "./VaultList.svelte";

vi.mock("../lib/api", () => ({
  deviceKind: vi.fn(),
  listVaults: vi.fn(),
  createVault: vi.fn(),
  unlockVault: vi.fn(),
  serverStatus: vi.fn(),
  syncServer: vi.fn(),
  listInvitations: vi.fn(),
}));

const created = { vault: { name: "perso", entries: 0 }, recoveryKey: "AAAA-BBBB" };

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(deviceKind).mockResolvedValue("desktop");
  vi.mocked(serverStatus).mockResolvedValue({ address: null, vaults: [] });
  vi.mocked(listInvitations).mockResolvedValue([]);
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

function expectVerbatim(element: HTMLElement) {
  expect(element).toHaveAttribute("autocapitalize", "off");
  expect(element).toHaveAttribute("autocorrect", "off");
  expect(element).toHaveAttribute("spellcheck", "false");
}

describe("VaultList", () => {
  it("opens a vault", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    const onOpen = vi.fn();
    render(VaultList, { onOpen, onBackup: vi.fn(), onRestore: vi.fn() });
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    expect(await screen.findByText("travail")).toHaveAttribute("title", "travail");
    await fireEvent.click(screen.getByRole("button", { name: "travail" }));
    expect(onOpen).toHaveBeenCalledWith("travail");
  });

  it("shows the notice, the empty list and the errors", async () => {
    vi.mocked(listVaults).mockResolvedValue([]);
    render(VaultList, {
      notice: "Coffre verrouillé.",
      onOpen: vi.fn(),
      onBackup: vi.fn(),
      onRestore: vi.fn(),
    });
    expect(screen.getByRole("status")).toHaveTextContent("Coffre verrouillé.");
    expect(await screen.findByText("Aucun coffre sur cet appareil.")).toBeInTheDocument();
  });

  it("shows a translated error", async () => {
    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
    expect(screen.queryByRole("button", { name: "Sauvegarder" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restaurer" })).toBeInTheDocument();
  });

  it("shows the state of the vaults only once connected, their type always", async () => {
    vi.mocked(listVaults).mockResolvedValue(["neuf", "perso", "vieux"]);
    vi.mocked(serverStatus).mockResolvedValue({
      address: null,
      vaults: [{ name: "neuf", state: "toPublish", shared: true }],
    });
    const { unmount } = render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    expect(await screen.findByText("commun")).toBeInTheDocument();
    expect(screen.queryByText("à publier")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();
    unmount();

    vi.mocked(serverStatus).mockResolvedValue({
      address: "pswd.local:8443",
      vaults: [
        { name: "neuf", state: "toPublish", shared: false },
        { name: "perso", state: "published", shared: false },
        { name: "vieux", state: "detached", shared: false },
      ],
    });
    render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    expect(await screen.findByText("à publier")).toBeInTheDocument();
    expect(screen.getByText("détaché")).toBeInTheDocument();
    expect(screen.queryByText("publié")).not.toBeInTheDocument();
  });

  it("exchanges vaults with the server, then lists them again", async () => {
    render(Toast);
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(serverStatus).mockResolvedValue({ address: "pswd.local:8443", vaults: [] });
    render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    vi.mocked(syncServer).mockResolvedValue({
      removed: [],
      downloaded: [{ name: "travail", localName: "travail" }],
      notDownloaded: [{ name: "famille", reason: "disque plein" }],
      published: [],
      notPublished: [],
    });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Coffre « travail » téléchargé.")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Coffre « famille » du serveur non téléchargé : disque plein.",
    );
    expect(await screen.findByRole("button", { name: "travail" })).toBeInTheDocument();

    vi.mocked(syncServer).mockResolvedValue({
      removed: [],
      downloaded: [],
      notDownloaded: [],
      published: [],
      notPublished: [{ name: "perso", reason: "refusé" }],
    });
    await fireEvent.click(screen.getByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Coffre « perso » non publié : refusé.")).toBeInTheDocument();

    vi.mocked(syncServer).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Échange avec le serveur impossible : serveur injoignable.")).toBeInTheDocument();
  });

  it("stays usable without a readable server state", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(serverStatus).mockRejectedValue({ code: "configInvalid", detail: "server.toml" });
    render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();
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
    fill("perso", "Maître-2026!", "Maitre-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    expect(createVault).not.toHaveBeenCalled();
  });

  it("states the password rule and refuses a weak one", async () => {
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    expect(
      screen.getByText(
        "Au moins 12 caractères, dont au moins 3 types parmi : minuscules, majuscules, chiffres, symboles.",
      ),
    ).toBeInTheDocument();
    fill("perso", "motdepasse12", "motdepasse12");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Le mot de passe maître doit mélanger au moins 3 types de caractères",
    );
    expect(createVault).not.toHaveBeenCalled();
  });

  it("creates the vault and clears the passwords", async () => {
    vi.mocked(createVault).mockResolvedValue(created);
    const onCreated = vi.fn();
    render(CreateVault, { onCreated, onCancel: vi.fn() });
    fill("perso", "Maître-2026!", "Maître-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    await vi.waitFor(() => {
      expect(onCreated).toHaveBeenCalledWith(created);
    });
    expect(createVault).toHaveBeenCalledWith("perso", "Maître-2026!", false);
    expect(field("Mot de passe maître").value).toBe("");
    expect(field("Confirmation").value).toBe("");
  });

  it("shows the backend refusal", async () => {
    vi.mocked(createVault).mockRejectedValue({ code: "vaultExists", detail: "perso" });
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    fill("perso", "Maître-2026!", "Maître-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Un coffre nommé perso existe déjà.",
    );
    expect(screen.getByRole("button", { name: "Créer" })).toBeEnabled();
  });

  it("keeps the name as typed on a phone keyboard", () => {
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    expectVerbatim(field("Nom du coffre"));
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
    await fireEvent.input(field("Mot de passe maître"), { target: { value: "Maître-2026!" } });
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 3 });
    });
    expect(unlockVault).toHaveBeenCalledWith("perso", "Maître-2026!");
    expect(field("Mot de passe maître").value).toBe("");
  });

  it("shows the typed password only on request", async () => {
    render(Unlock, { name: "perso", onUnlocked: vi.fn(), onCancel: vi.fn() });
    const password = field("Mot de passe maître");
    const reveal = screen.getByRole("button", { name: "Afficher le mot de passe" });
    expect(password).toHaveAttribute("type", "password");
    expect(reveal).toHaveAttribute("aria-pressed", "false");
    await fireEvent.click(reveal);
    expect(password).toHaveAttribute("type", "text");
    expect(reveal).toHaveAttribute("aria-pressed", "true");
    await fireEvent.click(reveal);
    expect(password).toHaveAttribute("type", "password");
    expect(unlockVault).not.toHaveBeenCalled();
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
