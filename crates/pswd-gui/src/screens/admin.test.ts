import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  addMember,
  adminAccounts,
  attachAccount,
  backupServerToFile,
  cancelKey,
  deleteAccount,
  deleteServerVault,
  pendingKeys,
  removeMember,
  restoreVersion,
  revokeDevice,
  serverBackupFileName,
  serverVaults,
  setAdmin,
  setPublicAddress,
  vaultVersions,
  type AdminAccount,
  type AdminVault,
} from "../lib/api";
import { pickServerBackupDestination } from "../lib/files";
import Toast from "../components/Toast.svelte";
import Admin from "./Admin.svelte";

vi.mock("../lib/api", () => ({
  adminAccounts: vi.fn(),
  addMember: vi.fn(),
  deleteAccount: vi.fn(),
  setAdmin: vi.fn(),
  revokeDevice: vi.fn(),
  pendingKeys: vi.fn(),
  cancelKey: vi.fn(),
  serverVaults: vi.fn(),
  deleteServerVault: vi.fn(),
  attachAccount: vi.fn(),
  removeMember: vi.fn(),
  vaultVersions: vi.fn(),
  restoreVersion: vi.fn(),
  setPublicAddress: vi.fn(),
  serverBackupFileName: vi.fn(),
  backupServerToFile: vi.fn(),
}));
vi.mock("../lib/files", () => ({ pickServerBackupDestination: vi.fn() }));

const MARCH_1 = Date.UTC(2026, 2, 1, 12);
const arthur: AdminAccount = { id: "a1", name: "Arthur", isAdmin: true, devices: [{ id: "d1", name: "PC", createdAt: MARCH_1 }] };
const camille: AdminAccount = { id: "a2", name: "Camille", isAdmin: false, devices: [] };
const famille: AdminVault = {
  id: "v1",
  name: "famille",
  shared: true,
  version: 3,
  members: [
    { id: "a1", name: "Arthur" },
    { id: "a2", name: "Camille" },
  ],
};
const abandonne: AdminVault = { id: "v2", name: "vieux", shared: false, version: null, members: [] };

beforeEach(() => {
  vi.resetAllMocks();
  render(Toast);
  vi.mocked(adminAccounts).mockResolvedValue([arthur, camille]);
  vi.mocked(pendingKeys).mockResolvedValue([]);
  vi.mocked(serverVaults).mockResolvedValue([famille, abandonne]);
  vi.mocked(vaultVersions).mockResolvedValue([]);
});

function show(publicAddress: string | null = null) {
  const onClose = vi.fn();
  render(Admin, { address: "pswd.local:8443", publicAddress, onClose });
  return onClose;
}

async function openTab(name: string) {
  await fireEvent.click(screen.getByRole("tab", { name }));
}

function card(name: string) {
  return within(screen.getByRole("heading", { name: new RegExp(`^${name}`) }).closest("li") as HTMLElement);
}

describe("Admin, accounts", () => {
  it("lists the accounts and their devices", async () => {
    const onClose = show();
    expect(await screen.findByText("PC, ajouté le 1 mars 2026")).toBeInTheDocument();
    expect(card("Arthur").getByText("administrateur")).toBeInTheDocument();
    expect(card("Camille").getByText("Aucun appareil connecté.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("revokes a device and changes roles", async () => {
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Révoquer PC" }));
    expect(revokeDevice).toHaveBeenCalledWith("d1");
    expect(await screen.findByText(/Appareil « PC » révoqué/)).toBeInTheDocument();
    await fireEvent.click(card("Camille").getByRole("button", { name: "Nommer administrateur" }));
    expect(setAdmin).toHaveBeenCalledWith("a2", true);
    expect(await screen.findByText("Camille est maintenant administrateur.")).toBeInTheDocument();
    const removeRole = () => card("Arthur").findByRole("button", { name: "Retirer le rôle d'administrateur" });
    await fireEvent.click(await removeRole());
    expect(setAdmin).toHaveBeenLastCalledWith("a1", false);
    expect(await screen.findByText("Arthur n'est plus administrateur.")).toBeInTheDocument();
    vi.mocked(setAdmin).mockRejectedValueOnce({ code: "serverError", detail: "dernier administrateur" });
    await fireEvent.click(await removeRole());
    expect(await screen.findByRole("alert")).toHaveTextContent("dernier administrateur");
  });

  it("deletes an account once its name is typed again", async () => {
    show();
    await screen.findByText("Camille");
    await fireEvent.click(card("Camille").getByRole("button", { name: "Supprimer le compte" }));
    const confirm = screen.getByRole("button", { name: "Supprimer" });
    expect(confirm).toBeDisabled();
    await fireEvent.input(screen.getByLabelText("Nom du compte"), { target: { value: "Camille" } });
    await fireEvent.click(confirm);
    expect(deleteAccount).toHaveBeenCalledWith("a2", "Camille", "Camille");
    expect(await screen.findByText("Compte « Camille » supprimé.")).toBeInTheDocument();

    await fireEvent.click(card("Camille").getByRole("button", { name: "Supprimer le compte" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByRole("button", { name: "Créer sa clé" })).toBeInTheDocument();
  });

  it("creates the key of a new member and shows it once", async () => {
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Créer sa clé" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Le nom du nouveau membre est obligatoire.");
    expect(addMember).not.toHaveBeenCalled();
    vi.mocked(addMember).mockResolvedValue("KKKK-LLLL");
    await fireEvent.input(screen.getByLabelText("Nom du nouveau membre"), { target: { value: " Dominique " } });
    await fireEvent.click(screen.getByRole("button", { name: "Créer sa clé" }));
    expect(addMember).toHaveBeenCalledWith("Dominique");
    expect(await screen.findByText("KKKK-LLLL")).toBeInTheDocument();
    expect(screen.getByText(/l'adresse du serveur \(pswd.local:8443\) et cette clé/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(screen.queryByText("KKKK-LLLL")).not.toBeInTheDocument();
  });

  it("explains an unreachable server", async () => {
    vi.mocked(adminAccounts).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    show();
    expect(await screen.findByRole("alert")).toHaveTextContent("serveur injoignable");
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
  });
});

describe("Admin, pending keys", () => {
  it("lists the keys and cancels one", async () => {
    vi.mocked(pendingKeys)
      .mockResolvedValueOnce([
        { id: "k1", purpose: "member", name: "Dominique", expiresAt: MARCH_1 },
        { id: "k2", purpose: "device", name: "Camille", expiresAt: MARCH_1 },
        { id: "k3", purpose: "admin", name: null, expiresAt: MARCH_1 },
      ])
      .mockResolvedValue([]);
    show();
    await openTab("Clés");
    expect(await screen.findByText("Nouveau membre : Dominique")).toBeInTheDocument();
    expect(screen.getByText("Appareil de plus pour Camille")).toBeInTheDocument();
    expect(screen.getByText("Clé administrateur")).toBeInTheDocument();
    expect(screen.getAllByText("expire le 1 mars 2026")).toHaveLength(3);
    const [first] = screen.getAllByRole("button", { name: "Annuler la clé" });
    await fireEvent.click(first as HTMLElement);
    expect(cancelKey).toHaveBeenCalledWith("k1");
    expect(await screen.findByText("Aucune clé de connexion en attente.")).toBeInTheDocument();
  });

  it("reports a failure", async () => {
    vi.mocked(pendingKeys).mockResolvedValue([{ id: "k1", purpose: "admin", name: null, expiresAt: MARCH_1 }]);
    vi.mocked(cancelKey).mockRejectedValue({ code: "serverError", detail: "clé introuvable" });
    show();
    await openTab("Clés");
    await fireEvent.click(await screen.findByRole("button", { name: "Annuler la clé" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("clé introuvable");
    vi.mocked(pendingKeys).mockRejectedValue({ code: "serverError", detail: "injoignable" });
    await openTab("Comptes");
    await openTab("Clés");
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
  });
});

describe("Admin, vaults", () => {
  it("lists the vaults of every account", async () => {
    show();
    await openTab("Coffres");
    const vault = await screen.findByRole("button", { name: "Gérer le coffre famille" });
    expect(vault).toHaveTextContent("commun · version 3");
    expect(vault).toHaveTextContent("Arthur, Camille");
    const old = screen.getByRole("button", { name: "Gérer le coffre vieux" });
    expect(old).toHaveTextContent("privé · jamais publié");
    expect(old).toHaveTextContent("Aucun membre.");
  });

  it("says when the server has no vault, or cannot be reached", async () => {
    vi.mocked(serverVaults).mockResolvedValueOnce([]).mockRejectedValue({ code: "serverError", detail: "injoignable" });
    show();
    await openTab("Coffres");
    expect(await screen.findByText("Aucun coffre sur le serveur.")).toBeInTheDocument();
    await openTab("Clés");
    await openTab("Coffres");
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
  });

  it("removes a member and restores a version", async () => {
    vi.mocked(vaultVersions).mockResolvedValue([
      { version: 3, createdAt: MARCH_1, size: 2048 },
      { version: 2, createdAt: MARCH_1, size: 1000 },
    ]);
    vi.mocked(restoreVersion).mockResolvedValue({ id: "v3", name: "famille-v2" });
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre famille" }));
    expect(await screen.findByText(/^Version 3 — 1 mars 2026 à \d+:00 — 2 Kio$/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer la version 2" }));
    expect(restoreVersion).toHaveBeenCalledWith("v1", 2);
    expect(await screen.findByText(/nouveau coffre « famille-v2 »/)).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole("button", { name: "Retirer Camille" }));
    expect(removeMember).toHaveBeenCalledWith("v1", "a2");
    expect(await screen.findByText("Camille a été retiré du coffre « famille ».")).toBeInTheDocument();
    vi.mocked(removeMember).mockRejectedValue({ code: "serverError", detail: "injoignable" });
    await fireEvent.click(await screen.findByRole("button", { name: "Retirer Arthur" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
    await fireEvent.click(screen.getByRole("button", { name: "Tous les coffres" }));
    expect(await screen.findByRole("button", { name: "Gérer le coffre vieux" })).toBeInTheDocument();
  });

  it("attaches an account to a vault without member", async () => {
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre vieux" }));
    expect(await screen.findByText("Aucune version publiée.")).toBeInTheDocument();
    await fireEvent.change(screen.getByLabelText("Compte à rattacher"), { target: { value: "a2" } });
    await fireEvent.click(screen.getByRole("button", { name: "Rattacher" }));
    expect(attachAccount).toHaveBeenCalledWith("v2", "a2");
    expect(await screen.findByText("Camille est maintenant membre du coffre « vieux ».")).toBeInTheDocument();
  });

  it("deletes a vault from the server once its name is typed again", async () => {
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre famille" }));
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer du serveur" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer du serveur" }));
    const confirm = screen.getByRole("button", { name: "Supprimer" });
    expect(confirm).toBeDisabled();
    vi.mocked(serverVaults).mockResolvedValue([abandonne]);
    await fireEvent.input(screen.getByLabelText("Nom du coffre"), { target: { value: "famille" } });
    await fireEvent.click(confirm);
    expect(deleteServerVault).toHaveBeenCalledWith("v1", "famille", "famille");
    expect(await screen.findByText("Coffre « famille » supprimé du serveur.")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Gérer le coffre vieux" })).toBeInTheDocument();
  });

  it("explains a vault that cannot be read", async () => {
    vi.mocked(vaultVersions).mockRejectedValue({ code: "serverError", detail: "coffre inconnu" });
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre vieux" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("coffre inconnu");
    expect(screen.queryByRole("button", { name: "Rattacher" })).not.toBeInTheDocument();
  });
});

describe("Admin, server", () => {
  it("saves the public address", async () => {
    show("maison:8443");
    await openTab("Serveur");
    const field = screen.getByLabelText("Adresse publique", { exact: false });
    expect(field).toHaveValue("maison:8443");
    await fireEvent.input(field, { target: { value: "" } });
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(setPublicAddress).toHaveBeenCalledWith("");
    expect(await screen.findByText(/Adresse publique enregistrée/)).toBeInTheDocument();
    vi.mocked(setPublicAddress).mockRejectedValue({ code: "serverError", detail: "adresse publique invalide" });
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("adresse publique invalide");
  });

  it("backs the server up into the chosen file", async () => {
    vi.mocked(serverBackupFileName).mockResolvedValue("pswd-server-sauvegarde.db");
    vi.mocked(pickServerBackupDestination).mockResolvedValueOnce(null).mockResolvedValue("E:/s.db");
    show();
    await openTab("Serveur");
    await fireEvent.click(screen.getByRole("button", { name: "Sauvegarder le serveur" }));
    expect(pickServerBackupDestination).toHaveBeenCalledWith(
      "Enregistrer la sauvegarde du serveur",
      "pswd-server-sauvegarde.db",
    );
    expect(backupServerToFile).not.toHaveBeenCalled();
    await fireEvent.click(await screen.findByRole("button", { name: "Sauvegarder le serveur" }));
    expect(await screen.findByText("Sauvegarde du serveur enregistrée.")).toBeInTheDocument();
    expect(backupServerToFile).toHaveBeenCalledWith("E:/s.db");
  });
});
