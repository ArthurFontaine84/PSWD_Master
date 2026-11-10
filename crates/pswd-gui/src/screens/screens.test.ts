import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createVault,
  deleteVaultEverywhere,
  deviceKind,
  leaveVault,
  listInvitations,
  listVaults,
  refreshServer,
  serverStatus,
  syncServer,
  unlockVault,
  vaultMembers,
  type Exchange,
  type ServerStatus,
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
  refreshServer: vi.fn(),
  serverStatus: vi.fn(),
  syncServer: vi.fn(),
  listInvitations: vi.fn(),
  vaultMembers: vi.fn(),
  leaveVault: vi.fn(),
  deleteVaultEverywhere: vi.fn(),
}));

const nothing: Exchange = {
  renamed: [],
  downloaded: [],
  notDownloaded: [],
  published: [],
  notPublished: [],
  sent: [],
  received: [],
  needsUnlock: [],
  notUpdated: [],
  removed: [],
  left: [],
  notLeft: [],
  deleted: [],
  notDeleted: [],
};

const created = { vault: { name: "perso", entries: 0 }, recoveryKey: "AAAA-BBBB" };

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(deviceKind).mockResolvedValue("desktop");
  vi.mocked(serverStatus).mockResolvedValue({ address: null, trouble: null, vaults: [] });
  vi.mocked(refreshServer).mockImplementation(() => serverStatus());
  vi.mocked(listInvitations).mockResolvedValue([]);
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

/** Les pastilles de la liste : un bouton chacune, nommé par son texte, coloré par sa pastille. */
function dots() {
  return screen
    .queryAllByRole("button")
    .filter((button) => button.classList.contains("dot"))
    .map((button) => [button.getAttribute("aria-label"), button.firstElementChild?.classList[1]]);
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
    expect(onOpen).toHaveBeenCalledWith("travail", null, false);
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
      trouble: null,
      vaults: [{ name: "neuf", state: null, shared: true }],
    });
    const { unmount } = render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    expect(await screen.findByText("commun")).toBeInTheDocument();
    expect(dots()).toEqual([]);
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();
    unmount();

    vi.mocked(serverStatus).mockResolvedValue({
      address: "pswd.local:8443",
      trouble: null,
      vaults: [
        { name: "neuf", state: "local", shared: false },
        { name: "perso", state: "synced", shared: false },
        { name: "vieux", state: "removed", shared: false },
      ],
    });
    const onOpen = vi.fn();
    render(VaultList, { onOpen, onBackup: vi.fn(), onRestore: vi.fn() });
    await screen.findByRole("button", { name: "Synchronisé" });
    expect(dots()).toEqual([
      ["Pas sur ce serveur : publié au prochain échange", "red"],
      ["Synchronisé", "green"],
      ["Absent de votre compte sur le serveur : gardé sur cet appareil", "red"],
    ]);
    await fireEvent.click(screen.getByRole("button", { name: "vieux" }));
    expect(onOpen).toHaveBeenCalledWith("vieux", "removed", false);
    await fireEvent.click(screen.getByRole("button", { name: "perso" }));
    expect(onOpen).toHaveBeenCalledWith("perso", "synced", false);
  });

  it("tells whether the vault opened is shared", async () => {
    vi.mocked(listVaults).mockResolvedValue(["famille"]);
    vi.mocked(serverStatus).mockResolvedValue({
      address: "pswd.local:8443",
      trouble: null,
      vaults: [{ name: "famille", state: "synced", shared: true }],
    });
    const onOpen = vi.fn();
    render(VaultList, { onOpen, onBackup: vi.fn(), onRestore: vi.fn() });
    await screen.findByRole("button", { name: "Synchronisé" });
    await fireEvent.click(screen.getByRole("button", { name: /^famille/ }));
    expect(onOpen).toHaveBeenCalledWith("famille", "synced", true);
  });

  it("tells the text of a dot when it is touched, without opening the vault", async () => {
    render(Toast);
    vi.mocked(listVaults).mockResolvedValue(["vieux"]);
    vi.mocked(serverStatus).mockResolvedValue({
      address: "pswd.local:8443",
      trouble: null,
      vaults: [{ name: "vieux", state: "removed", shared: false }],
    });
    const onOpen = vi.fn();
    render(VaultList, { onOpen, onBackup: vi.fn(), onRestore: vi.fn() });
    const dot = await screen.findByRole("button", { name: "Absent de votre compte sur le serveur : gardé sur cet appareil" });
    expect(dot).toHaveAttribute("title", "Absent de votre compte sur le serveur : gardé sur cet appareil");
    await fireEvent.click(dot);
    expect(await screen.findByRole("status")).toHaveTextContent("Absent de votre compte sur le serveur : gardé sur cet appareil");
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("exchanges vaults with the server, then lists them again", async () => {
    render(Toast);
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(serverStatus).mockResolvedValue({ address: "pswd.local:8443", trouble: null, vaults: [] });
    render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    vi.mocked(syncServer).mockResolvedValue({
      ...nothing,
      downloaded: [{ name: "travail", localName: "travail" }],
      notDownloaded: [{ name: "famille", reason: "disque plein" }],
    });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Coffre « travail » téléchargé.")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Coffre « famille » du serveur non téléchargé : disque plein.",
    );
    expect(await screen.findByRole("button", { name: "travail" })).toBeInTheDocument();

    vi.mocked(syncServer).mockResolvedValue({ ...nothing, notPublished: [{ name: "perso", reason: "refusé" }] });
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

describe("VaultList automatic exchange", () => {
  const connected: ServerStatus = {
    address: "pswd.local:8443",
    trouble: null,
    vaults: [{ name: "perso", state: "synced", shared: false }],
  };

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  function show() {
    return render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
  }

  it("exchanges at once, then every 30 seconds while the page is visible", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(refreshServer).mockResolvedValue(connected);
    const { unmount } = show();
    expect(await screen.findByRole("button", { name: "Synchronisé" })).toBeInTheDocument();
    expect(refreshServer).toHaveBeenCalledTimes(1);
    expect(serverStatus).not.toHaveBeenCalled();
    vi.advanceTimersByTime(30_000);
    expect(refreshServer).toHaveBeenCalledTimes(2);
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    vi.advanceTimersByTime(30_000);
    expect(refreshServer).toHaveBeenCalledTimes(2);
    visibility.mockReturnValue("visible");
    unmount();
    vi.advanceTimersByTime(30_000);
    expect(refreshServer).toHaveBeenCalledTimes(2);
  });

  it("exchanges again when the page comes back to the foreground", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(refreshServer).mockResolvedValue(connected);
    show();
    await screen.findByRole("button", { name: "Synchronisé" });
    await fireEvent(document, new Event("visibilitychange"));
    await fireEvent.focus(window);
    expect(refreshServer).toHaveBeenCalledTimes(3);
  });

  it("redraws the cards only when the names change", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(refreshServer).mockResolvedValue(connected);
    show();
    await vi.waitFor(() => {
      expect(listInvitations).toHaveBeenCalledTimes(1);
    });
    const card = screen.getByRole("button", { name: /perso/ });
    await fireEvent.focus(window);
    await vi.waitFor(() => {
      expect(listInvitations).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("button", { name: /perso/ })).toBe(card);

    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    await fireEvent.focus(window);
    expect(await screen.findByRole("button", { name: "travail" })).toBeInTheDocument();

    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    await fireEvent.focus(window);
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
  });

  it("announces the vaults and the invitations received by the automatic exchange", async () => {
    render(Toast);
    const invitation = { id: "1", vaultName: "famille", invitedBy: "Arthur" };
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(refreshServer).mockResolvedValue(connected);
    vi.mocked(listInvitations).mockResolvedValue([invitation]);
    show();
    expect(await screen.findByText("Arthur vous invite dans le coffre « famille ».")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();

    vi.mocked(listInvitations).mockResolvedValue([invitation, { id: "2", vaultName: "club", invitedBy: "Camille" }]);
    await fireEvent.focus(window);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Nouvelle invitation : Camille vous invite dans le coffre « club ».",
    );

    vi.mocked(listVaults).mockResolvedValue(["club", "perso"]);
    await fireEvent.focus(window);
    await vi.waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent("Nouveau coffre « club » reçu du serveur.");
    });

    vi.mocked(listVaults).mockResolvedValue(["club"]);
    vi.mocked(listInvitations).mockResolvedValue([]);
    await fireEvent.focus(window);
    await vi.waitFor(() => {
      expect(screen.queryByRole("button", { name: "perso" })).not.toBeInTheDocument();
    });
    expect(screen.getByRole("status")).toHaveTextContent("Nouveau coffre « club » reçu du serveur.");
  });

  it("announces a vault that gave its name away, not as a vault received", async () => {
    render(Toast);
    vi.mocked(listVaults).mockResolvedValue(["banque"]);
    vi.mocked(refreshServer).mockResolvedValue(connected);
    vi.mocked(listInvitations).mockResolvedValue([]);
    show();
    expect(await screen.findByRole("button", { name: "banque" })).toBeInTheDocument();

    vi.mocked(listVaults).mockResolvedValue(["banque", "banque-2"]);
    vi.mocked(refreshServer).mockResolvedValue({ ...connected, renamed: [{ name: "banque", newName: "banque-2" }] });
    await fireEvent.focus(window);
    expect(await screen.findByRole("button", { name: "banque-2" })).toBeInTheDocument();
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("Le serveur a déjà un coffre « banque » : celui de cet appareil devient « banque-2 ».");
    expect(status).not.toHaveTextContent("Nouveau coffre");
  });

  it("shows the vaults once they become readable, and keeps the error until then", async () => {
    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    vi.mocked(refreshServer).mockResolvedValue(connected);
    show();
    await vi.waitFor(() => {
      expect(listInvitations).toHaveBeenCalledTimes(1);
    });
    const error = screen.getByRole("alert");
    await fireEvent.focus(window);
    await vi.waitFor(() => {
      expect(listInvitations).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("alert")).toBe(error);

    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    await fireEvent.focus(window);
    expect(await screen.findByRole("button", { name: /perso/ })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ignores a busy or failing exchange, and an unreachable server", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso"]);
    vi.mocked(refreshServer).mockResolvedValueOnce(null);
    show();
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
    expect(refreshServer).toHaveBeenCalledTimes(1);

    vi.mocked(refreshServer).mockRejectedValueOnce({ code: "configInvalid", detail: "config.toml" });
    await fireEvent.focus(window);
    expect(refreshServer).toHaveBeenCalledTimes(2);
    expect(listVaults).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();

    vi.mocked(refreshServer).mockResolvedValueOnce({
      ...connected,
      trouble: { since: 0, reason: "serveur injoignable" },
      vaults: [{ name: "perso", state: null, shared: false }],
    });
    await fireEvent.focus(window);
    expect(await screen.findByRole("button", { name: "Synchroniser" })).toBeInTheDocument();
    expect(listInvitations).not.toHaveBeenCalled();
    expect(dots()).toEqual([["Échanges avec le serveur impossibles : synchro en pause", "red"]]);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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
    fill("  Mon coffre ", "Maître-2026!", "Maître-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    await vi.waitFor(() => {
      expect(onCreated).toHaveBeenCalledWith(created);
    });
    expect(createVault).toHaveBeenCalledWith("Mon coffre", "Maître-2026!", false);
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
    render(Unlock, { name: "perso", onUnlocked, onCancel: vi.fn(), onGone: vi.fn() });
    expect(screen.getByRole("heading")).toHaveTextContent("Déverrouiller « perso »");
    await fireEvent.input(field("Mot de passe maître"), { target: { value: "Maître-2026!" } });
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 3 });
    });
    expect(unlockVault).toHaveBeenCalledWith("perso", "Maître-2026!");
    expect(field("Mot de passe maître").value).toBe("");
  });

  it("shows the dot of the vault with its text", () => {
    const { unmount } = render(Unlock, {
      name: "perso",
      dot: "needsUnlock",
      onUnlocked: vi.fn(),
      onCancel: vi.fn(),
      onGone: vi.fn(),
    });
    const state = screen.getByText("Modifié ici et sur le serveur : à déverrouiller pour finir la synchro");
    expect(state.querySelector(".dot")).toHaveClass("orange");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    unmount();
    render(Unlock, { name: "perso", onUnlocked: vi.fn(), onCancel: vi.fn(), onGone: vi.fn() });
    expect(document.querySelector(".dot")).toBeNull();
  });

  it("tells that a vault deleted from the server only lives here", () => {
    const { unmount } = render(Unlock, { name: "perso", dot: "removed", onUnlocked: vi.fn(), onCancel: vi.fn(), onGone: vi.fn() });
    expect(screen.getByText("Absent de votre compte sur le serveur : gardé sur cet appareil").querySelector(".dot")).toHaveClass("red");
    expect(screen.getByRole("status")).toHaveTextContent("Ce coffre n'est pas (ou plus) sur votre compte du serveur");
    unmount();
    render(Unlock, { name: "perso", dot: "synced", onUnlocked: vi.fn(), onCancel: vi.fn(), onGone: vi.fn() });
    expect(screen.getByText("Synchronisé").querySelector(".dot")).toHaveClass("green");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the typed password only on request", async () => {
    render(Unlock, { name: "perso", onUnlocked: vi.fn(), onCancel: vi.fn(), onGone: vi.fn() });
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

  it("lets a member leave a published shared vault without its password", async () => {
    render(Toast);
    const members = [
      { id: "a1", name: "Arthur" },
      { id: "a2", name: "Camille" },
    ];
    vi.mocked(vaultMembers).mockResolvedValue({ members, invitations: [] });
    vi.mocked(leaveVault).mockResolvedValue();
    const onGone = vi.fn();
    render(Unlock, { name: "famille", dot: "outdated", shared: true, onUnlocked: vi.fn(), onCancel: vi.fn(), onGone });
    await fireEvent.click(screen.getByRole("button", { name: "Quitter ce coffre" }));
    expect(screen.queryByLabelText("Mot de passe maître", { exact: false })).not.toBeInTheDocument();
    await fireEvent.click(await screen.findByRole("button", { name: "Annuler" }));
    expect(field("Mot de passe maître")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Quitter ce coffre" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Quitter" }));
    expect(await screen.findByText("Vous avez quitté le coffre « famille ».")).toBeInTheDocument();
    expect(leaveVault).toHaveBeenCalledWith("famille");
    expect(onGone).toHaveBeenCalled();
    expect(unlockVault).not.toHaveBeenCalled();
  });

  it("lets the last member delete a shared vault without its password", async () => {
    render(Toast);
    vi.mocked(vaultMembers).mockResolvedValue({ members: [{ id: "a1", name: "Arthur" }], invitations: [] });
    vi.mocked(deleteVaultEverywhere).mockResolvedValue();
    const onGone = vi.fn();
    render(Unlock, { name: "famille", dot: "synced", shared: true, onUnlocked: vi.fn(), onCancel: vi.fn(), onGone });
    await fireEvent.click(screen.getByRole("button", { name: "Quitter ce coffre" }));
    await fireEvent.input(await screen.findByLabelText("Nom du coffre"), { target: { value: "famille" } });
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer" }));
    expect(await screen.findByText("Coffre « famille » supprimé.")).toBeInTheDocument();
    expect(deleteVaultEverywhere).toHaveBeenCalledWith("famille", "famille");
    expect(onGone).toHaveBeenCalled();
  });

  it("offers to leave only a published shared vault, outside recovery", async () => {
    const leave = () => screen.queryByRole("button", { name: "Quitter ce coffre" });
    const props = { onUnlocked: vi.fn(), onCancel: vi.fn(), onGone: vi.fn() };
    const absent = render(Unlock, { name: "famille", dot: "removed", shared: true, ...props });
    expect(leave()).not.toBeInTheDocument();
    absent.unmount();
    const offline = render(Unlock, { name: "famille", dot: "unreachable", shared: true, ...props });
    expect(leave()).not.toBeInTheDocument();
    offline.unmount();
    const personal = render(Unlock, { name: "perso", dot: "synced", ...props });
    expect(leave()).not.toBeInTheDocument();
    personal.unmount();
    render(Unlock, { name: "famille", dot: "needsUnlock", shared: true, ...props });
    expect(leave()).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Mot de passe oublié ?" }));
    expect(leave()).not.toBeInTheDocument();
  });

  it("explains a wrong password and can be cancelled", async () => {
    vi.mocked(unlockVault).mockRejectedValue({ code: "wrongPassword", detail: null });
    const onCancel = vi.fn();
    render(Unlock, { name: "perso", onUnlocked: vi.fn(), onCancel, onGone: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Mot de passe maître incorrect.");
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onCancel).toHaveBeenCalled();
  });
});
