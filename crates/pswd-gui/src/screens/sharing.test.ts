import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  acceptInvitation,
  createVault,
  deleteInvitation,
  deleteVaultEverywhere,
  inviteMember,
  leaveVault,
  listAccounts,
  listEntries,
  listInvitations,
  listTags,
  listVaults,
  onVaultSynced,
  refreshServer,
  serverStatus,
  syncServer,
  vaultMembers,
  type Exchange,
  type Invitation,
  type Members,
  type ServerStatus,
} from "../lib/api";
import Toast from "../components/Toast.svelte";
import CreateVault from "./CreateVault.svelte";
import OpenVault from "./OpenVault.svelte";
import VaultList from "./VaultList.svelte";

vi.mock("../lib/api", () => ({
  listVaults: vi.fn(),
  createVault: vi.fn(),
  refreshServer: vi.fn(),
  serverStatus: vi.fn(),
  syncServer: vi.fn(),
  listEntries: vi.fn(),
  listTags: vi.fn(),
  lockVault: vi.fn(),
  onVaultSynced: vi.fn(),
  listAccounts: vi.fn(),
  vaultMembers: vi.fn(),
  inviteMember: vi.fn(),
  listInvitations: vi.fn(),
  acceptInvitation: vi.fn(),
  deleteInvitation: vi.fn(),
  leaveVault: vi.fn(),
  deleteVaultEverywhere: vi.fn(),
}));

const arthur = { id: "a1", name: "Arthur" };
const camille = { id: "a2", name: "Camille" };
const dominique = { id: "a3", name: "Dominique" };
const invitation: Invitation = { id: "i1", vaultName: "famille", invitedBy: "Arthur" };
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
const shared: ServerStatus = {
  address: "pswd.local:8443",
  trouble: null,
  vaults: [
    { name: "famille", state: "synced", shared: true },
    { name: "ancien", state: "removed", shared: true },
    { name: "perso", state: "synced", shared: false },
  ],
};

beforeEach(() => {
  vi.resetAllMocks();
  render(Toast);
  vi.mocked(listVaults).mockResolvedValue(["ancien", "famille", "perso"]);
  vi.mocked(serverStatus).mockResolvedValue(shared);
  vi.mocked(refreshServer).mockImplementation(() => serverStatus());
  vi.mocked(listInvitations).mockResolvedValue([]);
  vi.mocked(listEntries).mockResolvedValue([]);
  vi.mocked(listTags).mockResolvedValue([]);
  vi.mocked(onVaultSynced).mockResolvedValue(vi.fn());
  vi.mocked(vaultMembers).mockResolvedValue({ members: [arthur, camille], invitations: [] });
});

function list() {
  render(VaultList, { onOpen: vi.fn(), onBackup: vi.fn(), onRestore: vi.fn() });
}

describe("creating a shared vault", () => {
  it("is a choice made at creation", async () => {
    vi.mocked(createVault).mockResolvedValue({ vault: { name: "famille", entries: 0 }, recoveryKey: "AAAA" });
    render(CreateVault, { onCreated: vi.fn(), onCancel: vi.fn() });
    await fireEvent.input(screen.getByLabelText("Nom du coffre", { exact: false }), { target: { value: "famille" } });
    for (const label of ["Mot de passe maître", "Confirmation"]) {
      await fireEvent.input(screen.getByLabelText(label, { exact: false }), { target: { value: "Maître-2026!" } });
    }
    expect(screen.getByText(/Ce choix est définitif\./)).toBeInTheDocument();
    await fireEvent.click(screen.getByLabelText("Coffre commun"));
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(createVault).toHaveBeenCalledWith("famille", "Maître-2026!", true);
  });
});

describe("the vault list", () => {
  it("marks the shared and the removed vaults", async () => {
    list();
    const card = (name: string) => within(screen.getByRole("button", { name: new RegExp(`^${name}`) }));
    await vi.waitFor(() => {
      expect(card("famille").getByText("commun")).toBeInTheDocument();
    });
    expect(card("ancien").getByText("commun")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Absent de votre compte sur le serveur : gardé sur cet appareil" })).toBeInTheDocument();
    expect(card("perso").queryByText("commun")).not.toBeInTheDocument();
  });

  it("shows the invitations first, and accepts one", async () => {
    vi.mocked(listInvitations).mockResolvedValueOnce([invitation]).mockResolvedValue([]);
    vi.mocked(acceptInvitation).mockResolvedValue({
      ...nothing,
      downloaded: [{ name: "famille", localName: "famille" }],
      notDownloaded: [{ name: "abime", reason: "disque plein" }],
    });
    list();
    expect(await screen.findByText("Arthur vous invite dans le coffre « famille ».")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Accepter" }));
    expect(
      await screen.findByText(
        "Coffre « famille » téléchargé. Demandez à Arthur le mot de passe maître du coffre pour l'ouvrir.",
      ),
    ).toBeInTheDocument();
    expect(acceptInvitation).toHaveBeenCalledWith("i1");
    expect(screen.getByRole("alert")).toHaveTextContent("Coffre « abime » du serveur non téléchargé : disque plein.");
    await vi.waitFor(() => {
      expect(screen.queryByText(/vous invite/)).not.toBeInTheDocument();
    });
    expect(listVaults).toHaveBeenCalledTimes(3);
  });

  it("declines an invitation", async () => {
    vi.mocked(listInvitations).mockResolvedValueOnce([invitation]).mockResolvedValue([]);
    list();
    await fireEvent.click(await screen.findByRole("button", { name: "Refuser" }));
    expect(await screen.findByText("Invitation au coffre « famille » refusée.")).toBeInTheDocument();
    expect(deleteInvitation).toHaveBeenCalledWith("i1");
    expect(screen.queryByText(/vous invite/)).not.toBeInTheDocument();
  });

  it("reports the errors of the invitations", async () => {
    vi.mocked(listInvitations).mockResolvedValue([invitation]);
    vi.mocked(acceptInvitation).mockRejectedValue({ code: "serverError", detail: "invitation inconnue" });
    vi.mocked(deleteInvitation).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    list();
    await fireEvent.click(await screen.findByRole("button", { name: "Accepter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Échange avec le serveur impossible : invitation inconnue.",
    );
    await fireEvent.click(screen.getByRole("button", { name: "Refuser" }));
    expect(await screen.findByText("Échange avec le serveur impossible : serveur injoignable.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refuser" })).toBeEnabled();
  });

  it("stays usable when the invitations cannot be read, silently until asked", async () => {
    vi.mocked(listInvitations).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    vi.mocked(syncServer).mockResolvedValue(nothing);
    list();
    await vi.waitFor(() => {
      expect(listInvitations).toHaveBeenCalled();
    });
    expect(screen.getByRole("button", { name: /^perso/ })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("serveur injoignable");
  });
});

/** Coffre `name` ouvert, avec le menu de gestion déplié. */
async function manage(name: string) {
  render(OpenVault, { vault: { name, entries: 0 }, onLocked: vi.fn(), onExpired: vi.fn(), onDeleted: vi.fn() });
  await vi.waitFor(() => {
    expect(serverStatus).toHaveBeenCalled();
  });
  await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
}

describe("the members of a shared vault", () => {
  it("are managed only for a published shared vault", async () => {
    await manage("perso");
    expect(screen.queryByRole("menuitem", { name: "Membres" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Quitter ce coffre" })).not.toBeInTheDocument();
  });

  it("are listed with the pending invitations, and more can be invited", async () => {
    vi.mocked(vaultMembers)
      .mockResolvedValueOnce({ members: [arthur], invitations: [{ id: "i1", account: camille }] })
      .mockResolvedValue({
        members: [arthur],
        invitations: [
          { id: "i1", account: camille },
          { id: "i2", account: dominique },
        ],
      });
    vi.mocked(listAccounts).mockResolvedValue([arthur, camille, dominique]);
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Membres" }));
    const members = await screen.findByRole("list", { name: "Membres" });
    expect(within(members).getByText("Arthur")).toBeInTheDocument();
    const pending = screen.getByRole("list", { name: "Invitations en attente" });
    expect(within(pending).getByText("Camille")).toBeInTheDocument();
    const choices = within(screen.getByLabelText("Inviter un compte")).getAllByRole("option");
    expect(choices.map((option) => option.textContent)).toEqual(["Dominique"]);
    await fireEvent.click(screen.getByRole("button", { name: "Inviter" }));
    expect(inviteMember).toHaveBeenCalledWith("famille", "a3");
    expect(
      await screen.findByText(
        "Invitation envoyée à Dominique. Transmettez-lui le mot de passe maître du coffre, sans passer par le serveur.",
      ),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("Tous les comptes du serveur sont déjà membres ou invités."),
    ).toBeInTheDocument();
  });

  it("can have an invitation cancelled", async () => {
    vi.mocked(vaultMembers)
      .mockResolvedValueOnce({ members: [arthur], invitations: [{ id: "i1", account: camille }] })
      .mockResolvedValue({ members: [arthur], invitations: [] });
    vi.mocked(listAccounts).mockResolvedValue([arthur, camille]);
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Membres" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Annuler l'invitation de Camille" }));
    expect(deleteInvitation).toHaveBeenCalledWith("i1");
    expect(await screen.findByText("Invitation de Camille annulée.")).toBeInTheDocument();
    await vi.waitFor(() => {
      expect(screen.queryByRole("list", { name: "Invitations en attente" })).not.toBeInTheDocument();
    });
    expect(screen.getByLabelText("Inviter un compte")).toHaveValue("a2");
  });

  it("reports the errors, then closes", async () => {
    vi.mocked(vaultMembers).mockRejectedValueOnce({ code: "notPublished", detail: "famille" });
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Membres" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Le coffre « famille » n'est pas publié sur le serveur de cet appareil.",
    );
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("reports a refused invitation", async () => {
    vi.mocked(vaultMembers).mockResolvedValue({ members: [arthur], invitations: [] });
    vi.mocked(listAccounts).mockResolvedValue([arthur, camille]);
    vi.mocked(inviteMember).mockRejectedValue({ code: "serverError", detail: "déjà invité" });
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Membres" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Inviter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Échange avec le serveur impossible : déjà invité.");
    expect(screen.getByRole("button", { name: "Inviter" })).toBeEnabled();
  });
});

describe("leaving a shared vault", () => {
  it("deletes the vault from the device and goes back to the list", async () => {
    vi.mocked(leaveVault).mockRejectedValueOnce({ code: "serverError", detail: "serveur injoignable" }).mockResolvedValue();
    const onLocked = vi.fn();
    render(OpenVault, { vault: { name: "famille", entries: 0 }, onLocked, onExpired: vi.fn(), onDeleted: vi.fn() });
    await vi.waitFor(() => {
      expect(serverStatus).toHaveBeenCalled();
    });
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Quitter ce coffre" }));
    expect(screen.getByText(/sera supprimé de cet appareil et de vos autres appareils/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Quitter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("serveur injoignable");
    expect(onLocked).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Quitter" }));
    expect(await screen.findByText("Vous avez quitté le coffre « famille ».")).toBeInTheDocument();
    expect(leaveVault).toHaveBeenCalledWith("famille");
    expect(onLocked).toHaveBeenCalled();
  });

  it("can be cancelled", async () => {
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Quitter ce coffre" }));
    await fireEvent.click(await screen.findByRole("button", { name: "Annuler" }));
    expect(leaveVault).not.toHaveBeenCalled();
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("is not offered as a deletion from this device only", async () => {
    await manage("famille");
    expect(screen.queryByRole("menuitem", { name: "Supprimer ce coffre" })).not.toBeInTheDocument();
  });

  it("is still offered when the members cannot be read", async () => {
    vi.mocked(vaultMembers).mockRejectedValue({ code: "serverUnreachable", detail: null });
    await manage("famille");
    await fireEvent.click(screen.getByRole("menuitem", { name: "Quitter ce coffre" }));
    expect(await screen.findByRole("button", { name: "Quitter" })).toBeEnabled();
  });

  it("becomes a deletion everywhere for the last member", async () => {
    let resolve: (members: Members) => void = () => undefined;
    vi.mocked(vaultMembers).mockReturnValue(new Promise((done) => (resolve = done)));
    vi.mocked(deleteVaultEverywhere)
      .mockRejectedValueOnce({ code: "serverUnreachable", detail: null })
      .mockResolvedValue();
    const onDeleted = vi.fn();
    render(OpenVault, { vault: { name: "famille", entries: 0 }, onLocked: vi.fn(), onExpired: vi.fn(), onDeleted });
    await vi.waitFor(() => {
      expect(serverStatus).toHaveBeenCalled();
    });
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Quitter ce coffre" }));
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    resolve({ members: [arthur], invitations: [] });
    expect(await screen.findByText(/Vous êtes le dernier membre du coffre « famille »/)).toBeInTheDocument();
    expect(vaultMembers).toHaveBeenCalledWith("famille");
    expect(screen.queryByRole("button", { name: "Quitter" })).not.toBeInTheDocument();
    const confirm = screen.getByRole("button", { name: "Supprimer" });
    expect(confirm).toBeDisabled();
    await fireEvent.input(screen.getByLabelText("Nom du coffre"), { target: { value: "famille" } });
    await fireEvent.click(confirm);
    expect(await screen.findByRole("alert")).toHaveTextContent("Serveur injoignable.");
    expect(onDeleted).not.toHaveBeenCalled();
    await fireEvent.click(confirm);
    await vi.waitFor(() => {
      expect(onDeleted).toHaveBeenCalledWith("Coffre « famille » supprimé.");
    });
    expect(deleteVaultEverywhere).toHaveBeenCalledWith("famille", "famille");
    expect(leaveVault).not.toHaveBeenCalled();
  });
});
