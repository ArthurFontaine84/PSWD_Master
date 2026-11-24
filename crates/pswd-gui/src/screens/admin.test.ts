import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

/** Le panneau, vu depuis un appareil du compte d'Arthur. */
function show(publicAddress: string | null = null) {
  const onClose = vi.fn();
  const onDisconnected = vi.fn();
  render(Admin, { address: "pswd.local:8443", publicAddress, accountId: "a1", onClose, onDisconnected });
  return Object.assign(onClose, { onDisconnected });
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

  it("revokes a device once confirmed, never the last one of the last administrator", async () => {
    const phone = { id: "d2", name: "Téléphone", createdAt: MARCH_1 };
    vi.mocked(adminAccounts).mockResolvedValue([arthur, { ...camille, devices: [phone] }]);
    show();
    const revoke = await screen.findByRole("button", { name: "Révoquer Téléphone" });
    expect(revoke).toHaveTextContent(/^Révoquer$/);
    expect(screen.queryByRole("button", { name: "Révoquer PC" })).not.toBeInTheDocument();

    await fireEvent.click(revoke);
    expect(screen.getByRole("heading", { name: "Révoquer l'appareil" })).toBeInTheDocument();
    expect(screen.getByText(/« Téléphone » \(compte Camille\) n'aura plus accès au serveur/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retour" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(revokeDevice).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Révoquer Téléphone" }));
    vi.mocked(revokeDevice).mockRejectedValueOnce({ code: "serverError", detail: "injoignable" });
    await fireEvent.click(screen.getByRole("button", { name: "Révoquer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
    const laptop = { id: "d3", name: "Portable", createdAt: MARCH_1 };
    vi.mocked(adminAccounts).mockResolvedValue([{ ...arthur, devices: [...arthur.devices, laptop] }, camille]);
    await fireEvent.click(screen.getByRole("button", { name: "Révoquer" }));
    expect(revokeDevice).toHaveBeenLastCalledWith("d2");
    expect(await screen.findByText("« Téléphone » révoqué.")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Révoquer l'appareil" })).not.toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Révoquer Portable" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Révoquer PC" })).toBeInTheDocument();

    vi.mocked(adminAccounts).mockResolvedValue([arthur, camille]);
    await fireEvent.click(screen.getByRole("button", { name: "Révoquer Portable" }));
    await fireEvent.click(screen.getByRole("button", { name: "Révoquer" }));
    expect(await screen.findByText("« Portable » révoqué.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Révoquer PC" })).not.toBeInTheDocument();

    vi.mocked(adminAccounts).mockResolvedValue([arthur, { ...camille, isAdmin: true }]);
    await fireEvent.click(card("Camille").getByRole("button", { name: "Nommer administrateur" }));
    expect(await screen.findByRole("button", { name: "Révoquer PC" })).toBeInTheDocument();
  });

  it("changes roles, but keeps the last administrator", async () => {
    const lastAdmin = () => card("Arthur").queryByText("Dernier administrateur");
    const removeRole = () => card("Arthur").queryByRole("button", { name: "Retirer le rôle d'administrateur" });
    show();
    await screen.findByText("Camille");
    expect(lastAdmin()).toBeInTheDocument();
    expect(removeRole()).not.toBeInTheDocument();

    vi.mocked(adminAccounts).mockResolvedValue([arthur, { ...camille, isAdmin: true }]);
    await fireEvent.click(card("Camille").getByRole("button", { name: "Nommer administrateur" }));
    expect(setAdmin).toHaveBeenCalledWith("a2", true);
    expect(await screen.findByText("Camille est maintenant administrateur.")).toBeInTheDocument();
    expect(lastAdmin()).not.toBeInTheDocument();

    vi.mocked(setAdmin).mockRejectedValueOnce({ code: "serverError", detail: "refusé" });
    await fireEvent.click(removeRole() as HTMLElement);
    expect(await screen.findByRole("alert")).toHaveTextContent("refusé");

    vi.mocked(adminAccounts).mockResolvedValue([{ ...arthur, isAdmin: false }, { ...camille, isAdmin: true }]);
    await fireEvent.click(removeRole() as HTMLElement);
    expect(setAdmin).toHaveBeenLastCalledWith("a1", false);
    expect(await screen.findByText("Arthur n'est plus administrateur.")).toBeInTheDocument();
    expect(card("Camille").getByText("Dernier administrateur")).toBeInTheDocument();
    expect(card("Arthur").getByRole("button", { name: "Nommer administrateur" })).toBeInTheDocument();
  });

  it("deletes the own account of an administrator who is not the last one, never another administrator", async () => {
    vi.mocked(adminAccounts).mockResolvedValue([arthur, { ...camille, isAdmin: true }]);
    const { onDisconnected } = show();
    await screen.findByText("Camille");
    expect(card("Arthur").getByRole("button", { name: "Retirer le rôle d'administrateur" })).toBeInTheDocument();
    expect(card("Arthur").queryByRole("button", { name: "Supprimer le compte" })).not.toBeInTheDocument();
    expect(card("Camille").getByRole("button", { name: "Retirer le rôle d'administrateur" })).toBeInTheDocument();
    expect(card("Camille").queryByRole("button", { name: /^Supprimer/ })).not.toBeInTheDocument();

    await fireEvent.click(card("Arthur").getByRole("button", { name: "Supprimer mon compte" }));
    expect(screen.getByRole("heading", { name: "Supprimer mon compte" })).toBeInTheDocument();
    expect(screen.getByText(/cet appareil déconnecté ; vos coffres restent ici/)).toBeInTheDocument();
    vi.mocked(deleteAccount).mockResolvedValue(true);
    await fireEvent.input(screen.getByLabelText("Nom du compte"), { target: { value: "Arthur" } });
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer" }));
    expect(deleteAccount).toHaveBeenCalledWith("a1", "Arthur", "Arthur");
    expect(await screen.findByText("Compte supprimé : appareil déconnecté.")).toBeInTheDocument();
    expect(onDisconnected).toHaveBeenCalled();
    expect(adminAccounts).toHaveBeenCalledTimes(1);
  });

  it("deletes an account once its name is typed again", async () => {
    show();
    await screen.findByText("Camille");
    await fireEvent.click(card("Camille").getByRole("button", { name: "Supprimer le compte" }));
    const confirm = screen.getByRole("button", { name: "Supprimer" });
    expect(confirm).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Retour" })).not.toBeInTheDocument();
    await fireEvent.input(screen.getByLabelText("Nom du compte"), { target: { value: "Camille" } });
    vi.mocked(deleteAccount).mockResolvedValue(false);
    await fireEvent.click(confirm);
    expect(deleteAccount).toHaveBeenCalledWith("a2", "Camille", "Camille");
    expect(await screen.findByText("Compte « Camille » supprimé.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();

    await fireEvent.click(card("Camille").getByRole("button", { name: "Supprimer le compte" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByRole("button", { name: "Ajouter un membre" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();
  });

  it("shows the back button again when the deletion is left for another tab", async () => {
    show();
    await screen.findByText("Camille");
    await fireEvent.click(card("Camille").getByRole("button", { name: "Supprimer le compte" }));
    expect(screen.queryByRole("button", { name: "Retour" })).not.toBeInTheDocument();
    await openTab("Clés");
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();
  });

  it("opens the form of a new member, then closes it", async () => {
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Ajouter un membre" }));
    expect(screen.queryByRole("button", { name: "Ajouter un membre" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Le nom du nouveau membre est obligatoire.");
    await fireEvent.input(screen.getByLabelText("Nom du nouveau membre"), { target: { value: "Zoé" } });
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.queryByLabelText("Nom du nouveau membre")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Ajouter un membre" }));
    expect(screen.getByLabelText("Nom du nouveau membre")).toHaveValue("");
    expect(addMember).not.toHaveBeenCalled();
  });

  it("creates the key of a new member and shows it once", async () => {
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Ajouter un membre" }));
    vi.mocked(addMember).mockResolvedValue("KKKK-LLLL");
    await fireEvent.input(screen.getByLabelText("Nom du nouveau membre"), { target: { value: " Dominique " } });
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(addMember).toHaveBeenCalledWith("Dominique");
    expect(await screen.findByText("KKKK-LLLL")).toBeInTheDocument();
    expect(screen.getByText(/l'adresse du serveur \(pswd.local:8443\) et cette clé/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(screen.queryByText("KKKK-LLLL")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ajouter un membre" })).toBeInTheDocument();
  });

  it("refuses the name of an account or of a member still waiting", async () => {
    vi.mocked(pendingKeys).mockResolvedValue([{ id: "k1", purpose: "member", name: "Dominique", expiresAt: MARCH_1 }]);
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Ajouter un membre" }));
    for (const name of ["Camille", "Dominique"]) {
      await fireEvent.input(screen.getByLabelText("Nom du nouveau membre"), { target: { value: name } });
      await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
      expect(screen.getByRole("alert")).toHaveTextContent("Ce nom est déjà pris.");
    }
    expect(addMember).not.toHaveBeenCalled();
    vi.mocked(addMember).mockRejectedValue({ code: "serverError", detail: "ce nom est déjà pris" });
    await fireEvent.input(screen.getByLabelText("Nom du nouveau membre"), { target: { value: "camille" } });
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(addMember).toHaveBeenCalledWith("camille");
    expect(await screen.findByRole("alert")).toHaveTextContent("ce nom est déjà pris");
    expect(screen.getByLabelText("Nom du nouveau membre")).toHaveValue("camille");
  });

  it("lists the members whose key is not used yet, and cancels such a key", async () => {
    vi.mocked(pendingKeys).mockResolvedValue([
      { id: "k1", purpose: "member", name: "Dominique", expiresAt: MARCH_1 },
      { id: "k2", purpose: "device", name: "Camille", expiresAt: MARCH_1 },
    ]);
    show();
    await screen.findByText("Camille");
    expect(card("Dominique").getByText("en attente")).toBeInTheDocument();
    expect(card("Dominique").getByText("Clé pas encore utilisée")).toBeInTheDocument();
    expect(card("Camille").queryByText("en attente")).not.toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 3, name: /^(Arthur|Camille|Dominique)/ })).toHaveLength(3);

    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "dom" } });
    expect(screen.queryByRole("heading", { name: /^Camille/ })).not.toBeInTheDocument();
    vi.mocked(pendingKeys).mockResolvedValue([]);
    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé de Dominique" }));
    expect(screen.getByText("La clé « Nouveau membre : Dominique » ne pourra plus servir.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Garder la clé" }));
    expect(cancelKey).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé de Dominique" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé" }));
    expect(cancelKey).toHaveBeenCalledWith("k1");
    expect(await screen.findByText("Clé annulée.")).toBeInTheDocument();
    expect(screen.getByText("Aucun compte ne porte ce nom.")).toBeInTheDocument();
  });

  it("explains an unreachable server", async () => {
    vi.mocked(adminAccounts).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    show();
    expect(await screen.findByRole("alert")).toHaveTextContent("serveur injoignable");
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
  });

  it("follows the changes made elsewhere, silently, except during a confirmation", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    show();
    await screen.findByText("Camille");
    const phone = { id: "d2", name: "Téléphone", createdAt: MARCH_1 };
    vi.mocked(adminAccounts).mockResolvedValue([arthur, { ...camille, devices: [phone] }]);
    vi.advanceTimersByTime(5_000);
    expect(await screen.findByText("Téléphone, ajouté le 1 mars 2026")).toBeInTheDocument();

    vi.mocked(adminAccounts).mockRejectedValue({ code: "serverUnreachable", detail: null });
    vi.advanceTimersByTime(5_000);
    await vi.waitFor(() => {
      expect(adminAccounts).toHaveBeenCalledTimes(3);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("Téléphone, ajouté le 1 mars 2026")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Révoquer Téléphone" }));
    vi.advanceTimersByTime(5_000);
    expect(adminAccounts).toHaveBeenCalledTimes(3);
    vi.useRealTimers();
  });
});

describe("Admin, pending keys", () => {
  it("lists the keys and cancels one once confirmed", async () => {
    vi.mocked(pendingKeys).mockResolvedValue([
      { id: "k1", purpose: "member", name: "Dominique", expiresAt: MARCH_1 },
      { id: "k2", purpose: "device", name: "Camille", expiresAt: MARCH_1 },
      { id: "k3", purpose: "admin", name: null, expiresAt: MARCH_1 },
    ]);
    show();
    await openTab("Clés");
    expect(await screen.findByText("Nouveau membre : Dominique")).toBeInTheDocument();
    expect(screen.getByText("Appareil de plus pour Camille")).toBeInTheDocument();
    expect(screen.getByText("Clé administrateur")).toBeInTheDocument();
    expect(screen.getAllByText("expire le 1 mars 2026")).toHaveLength(3);
    const [, second] = screen.getAllByRole("button", { name: "Annuler la clé" });
    await fireEvent.click(second as HTMLElement);
    expect(screen.getByText("La clé « Appareil de plus pour Camille » ne pourra plus servir.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retour" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Garder la clé" }));
    expect(cancelKey).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();

    vi.mocked(pendingKeys).mockResolvedValue([]);
    const [first] = screen.getAllByRole("button", { name: "Annuler la clé" });
    await fireEvent.click(first as HTMLElement);
    expect(screen.getByText("La clé « Nouveau membre : Dominique » ne pourra plus servir.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé" }));
    expect(cancelKey).toHaveBeenCalledWith("k1");
    expect(await screen.findByText("Aucune clé de connexion en attente.")).toBeInTheDocument();
  });

  it("reports a failure", async () => {
    vi.mocked(pendingKeys).mockResolvedValue([{ id: "k1", purpose: "admin", name: null, expiresAt: MARCH_1 }]);
    vi.mocked(cancelKey).mockRejectedValue({ code: "serverError", detail: "clé introuvable" });
    show();
    await openTab("Clés");
    await fireEvent.click(await screen.findByRole("button", { name: "Annuler la clé" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("clé introuvable");
    expect(screen.getByText("La clé « Clé administrateur » ne pourra plus servir.")).toBeInTheDocument();
    vi.mocked(pendingKeys).mockRejectedValue({ code: "serverError", detail: "injoignable" });
    await openTab("Comptes");
    await openTab("Clés");
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
  });
});

describe("Admin, pending keys, following the server", () => {
  it("drops a key used elsewhere, silently, except during a confirmation", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    const dominique = { id: "k1", purpose: "member", name: "Dominique", expiresAt: MARCH_1 } as const;
    const zoe = { id: "k2", purpose: "member", name: "Zoé", expiresAt: MARCH_1 } as const;
    vi.mocked(pendingKeys).mockResolvedValue([dominique, zoe]);
    show();
    await openTab("Clés");
    await screen.findByText("Nouveau membre : Zoé");
    const calls = vi.mocked(pendingKeys).mock.calls.length;

    vi.mocked(pendingKeys).mockResolvedValue([zoe]);
    vi.advanceTimersByTime(5_000);
    await vi.waitFor(() => {
      expect(screen.queryByText("Nouveau membre : Dominique")).not.toBeInTheDocument();
    });

    vi.mocked(pendingKeys).mockRejectedValue({ code: "serverUnreachable", detail: null });
    vi.advanceTimersByTime(5_000);
    await vi.waitFor(() => {
      expect(pendingKeys).toHaveBeenCalledTimes(calls + 2);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Annuler la clé" }));
    vi.advanceTimersByTime(5_000);
    expect(pendingKeys).toHaveBeenCalledTimes(calls + 2);
    vi.useRealTimers();
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
    expect(screen.getByText("(actuelle)")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Restaurer la version 3" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer la version 2" }));
    expect(restoreVersion).toHaveBeenCalledWith("v1", 2);
    expect(await screen.findByText("Version restaurée dans « famille-v2 ».")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retirer Arthur" })).not.toBeInTheDocument();
    vi.mocked(removeMember).mockRejectedValueOnce({ code: "serverError", detail: "injoignable" });
    const remove = await screen.findByRole("button", { name: "Retirer Camille" });
    expect(remove).toHaveTextContent(/^Retirer$/);
    await fireEvent.click(remove);
    expect(await screen.findByRole("alert")).toHaveTextContent("injoignable");
    await fireEvent.click(await screen.findByRole("button", { name: "Retirer Camille" }));
    expect(removeMember).toHaveBeenLastCalledWith("v1", "a2");
    expect(await screen.findByText("Camille retiré de « famille ».")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Tous les coffres" }));
    expect(await screen.findByRole("button", { name: "Gérer le coffre vieux" })).toBeInTheDocument();
  });

  it("does not remove the owner of a private vault", async () => {
    vi.mocked(serverVaults).mockResolvedValue([{ ...abandonne, version: 1, members: [{ id: "a2", name: "Camille" }] }]);
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre vieux" }));
    expect(await screen.findByText("Camille")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retirer Camille" })).not.toBeInTheDocument();
  });

  it("attaches an account to a vault without member", async () => {
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre vieux" }));
    expect(await screen.findByText("Aucune version publiée.")).toBeInTheDocument();
    await fireEvent.change(screen.getByLabelText("Compte à rattacher"), { target: { value: "a2" } });
    await fireEvent.click(screen.getByRole("button", { name: "Rattacher" }));
    expect(attachAccount).toHaveBeenCalledWith("v2", "a2");
    expect(await screen.findByText("Camille ajouté à « vieux ».")).toBeInTheDocument();
  });

  it("deletes a vault from the server once its name is typed again", async () => {
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre famille" }));
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer du serveur" }));
    expect(screen.queryByRole("button", { name: "Retour" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer du serveur" }));
    const confirm = screen.getByRole("button", { name: "Supprimer" });
    expect(confirm).toBeDisabled();
    vi.mocked(serverVaults).mockResolvedValue([abandonne]);
    await fireEvent.input(screen.getByLabelText("Nom du coffre"), { target: { value: "famille" } });
    await fireEvent.click(confirm);
    expect(deleteServerVault).toHaveBeenCalledWith("v1", "famille", "famille");
    expect(await screen.findByText("Coffre « famille » supprimé du serveur.")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Gérer le coffre vieux" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retour" })).toBeInTheDocument();
  });

  it("follows the changes made elsewhere, keeping the open vault while it does not change", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre vieux" }));
    await fireEvent.change(await screen.findByLabelText("Compte à rattacher"), { target: { value: "a2" } });
    vi.advanceTimersByTime(5_000);
    await vi.waitFor(() => {
      expect(serverVaults).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByLabelText("Compte à rattacher")).toHaveValue("a2");
    expect(vaultVersions).toHaveBeenCalledTimes(1);

    vi.mocked(serverVaults).mockResolvedValue([famille]);
    vi.advanceTimersByTime(5_000);
    expect(await screen.findByRole("button", { name: "Gérer le coffre famille" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Gérer le coffre vieux" })).not.toBeInTheDocument();

    vi.mocked(serverVaults).mockRejectedValue({ code: "serverUnreachable", detail: null });
    vi.advanceTimersByTime(5_000);
    await vi.waitFor(() => {
      expect(serverVaults).toHaveBeenCalledTimes(4);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    vi.mocked(serverVaults).mockResolvedValue([famille]);
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre famille" }));
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer du serveur" }));
    vi.advanceTimersByTime(5_000);
    expect(serverVaults).toHaveBeenCalledTimes(4);
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    vi.advanceTimersByTime(5_000);
    expect(serverVaults).toHaveBeenCalledTimes(5);
    vi.useRealTimers();
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

describe("Admin, searching", () => {
  it("finds an account by its name", async () => {
    show();
    await screen.findByText("Camille");
    await fireEvent.input(screen.getByRole("searchbox", { name: "Rechercher un compte" }), {
      target: { value: "cam" },
    });
    expect(screen.getByRole("heading", { name: "Camille" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^Arthur/ })).not.toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "zoé" } });
    expect(screen.getByText("Aucun compte ne porte ce nom.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ajouter un membre" })).toBeInTheDocument();
  });

  it("finds a vault by its name", async () => {
    show();
    await openTab("Coffres");
    await fireEvent.input(await screen.findByRole("searchbox", { name: "Rechercher un coffre" }), {
      target: { value: "VIEUX" },
    });
    expect(screen.getByRole("button", { name: "Gérer le coffre vieux" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Gérer le coffre famille" })).not.toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "perso" } });
    expect(screen.getByText("Aucun coffre ne porte ce nom.")).toBeInTheDocument();
  });
});

describe("Admin, pages on a wide screen", () => {
  /** Fenêtre large mais basse : le minimum de lignes par page s'applique. */
  function wideScreen() {
    const query = { matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() };
    vi.stubGlobal("matchMedia", vi.fn(() => query));
    vi.stubGlobal("innerHeight", 400);
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const numbered = (count: number) => Array.from({ length: count }, (_, index) => String(index + 1));

  it("shows four accounts per page, and starts again from the first page on a new search", async () => {
    wideScreen();
    const others = numbered(9).map((n) => ({ ...camille, id: `c${n}`, name: `Compte ${n}` }));
    vi.mocked(adminAccounts).mockResolvedValue([arthur, ...others]);
    show();
    expect(await screen.findByText("Page 1 / 3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Compte 3" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Compte 4" })).not.toBeInTheDocument();
    const next = screen.getByRole("button", { name: "Page suivante" });
    await fireEvent.click(next);
    await fireEvent.click(next);
    expect(screen.getByText("Page 3 / 3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Compte 9" })).toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "compte" } });
    expect(screen.getByText("Page 1 / 3")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Compte 1" })).toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "compte 1" } });
    expect(screen.queryByRole("navigation", { name: "Pages des comptes" })).not.toBeInTheDocument();
  });

  it("shows four vaults per page, and starts again from the first page on a new search", async () => {
    wideScreen();
    vi.mocked(serverVaults).mockResolvedValue(numbered(5).map((n) => ({ ...famille, id: `v${n}`, name: `coffre ${n}` })));
    show();
    await openTab("Coffres");
    const pages = await screen.findByRole("navigation", { name: "Pages des coffres" });
    expect(pages).toHaveTextContent("Page 1 / 2");
    expect(screen.queryByRole("button", { name: "Gérer le coffre coffre 5" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Page suivante" }));
    expect(screen.getByRole("button", { name: "Gérer le coffre coffre 5" })).toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "coffre" } });
    expect(pages).toHaveTextContent("Page 1 / 2");
  });

  it("shows three keys per page", async () => {
    wideScreen();
    vi.mocked(pendingKeys).mockResolvedValue(
      numbered(4).map((n) => ({ id: `k${n}`, purpose: "member" as const, name: `Membre ${n}`, expiresAt: MARCH_1 })),
    );
    show();
    await openTab("Clés");
    const pages = await screen.findByRole("navigation", { name: "Pages des clés" });
    expect(pages).toHaveTextContent("Page 1 / 2");
    expect(screen.getByText("Nouveau membre : Membre 3")).toBeInTheDocument();
    expect(screen.queryByText("Nouveau membre : Membre 4")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Page suivante" }));
    expect(screen.getByText("Nouveau membre : Membre 4")).toBeInTheDocument();
  });

  it("shows three versions per page", async () => {
    wideScreen();
    vi.mocked(serverVaults).mockResolvedValue([{ ...famille, version: 4 }]);
    vi.mocked(vaultVersions).mockResolvedValue(
      [4, 3, 2, 1].map((version) => ({ version, createdAt: MARCH_1, size: 1024 })),
    );
    show();
    await openTab("Coffres");
    await fireEvent.click(await screen.findByRole("button", { name: "Gérer le coffre famille" }));
    const pages = await screen.findByRole("navigation", { name: "Pages des versions" });
    expect(pages).toHaveTextContent("Page 1 / 2");
    expect(screen.getByRole("button", { name: "Restaurer la version 2" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Restaurer la version 1" })).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Page suivante" }));
    expect(screen.getByRole("button", { name: "Restaurer la version 1" })).toBeInTheDocument();
  });
});

describe("Admin, server", () => {
  it("shows the public address as coming in a future version", async () => {
    show("maison:8443");
    await openTab("Serveur");
    const field = screen.getByLabelText("Adresse publique", { exact: false });
    expect(field).toHaveValue("maison:8443");
    expect(field).toBeDisabled();
    expect(screen.getByText(/arrivera dans une prochaine version/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Enregistrer" })).not.toBeInTheDocument();
    expect(setPublicAddress).not.toHaveBeenCalled();
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
