import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  backupContents,
  backupVaults,
  changePassword,
  deleteVault,
  getSettings,
  importEntries,
  listEntries,
  listTags,
  listVaults,
  recoverVault,
  restoreVault,
  saveSettings,
  unlockVault,
} from "../lib/api";
import { pickCsvFile, pickFolder } from "../lib/files";
import OpenVault from "./OpenVault.svelte";
import Restore from "./Restore.svelte";
import Settings from "./Settings.svelte";
import Unlock from "./Unlock.svelte";
import VaultList from "./VaultList.svelte";

vi.mock("../lib/api", () => ({
  listVaults: vi.fn(),
  listEntries: vi.fn(),
  listTags: vi.fn(),
  lockVault: vi.fn(),
  unlockVault: vi.fn(),
  recoverVault: vi.fn(),
  changePassword: vi.fn(),
  importEntries: vi.fn(),
  deleteVault: vi.fn(),
  backupVaults: vi.fn(),
  backupContents: vi.fn(),
  restoreVault: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
}));
vi.mock("../lib/files", () => ({ pickFolder: vi.fn(), pickCsvFile: vi.fn() }));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listVaults).mockResolvedValue(["perso"]);
  vi.mocked(listEntries).mockResolvedValue([]);
  vi.mocked(listTags).mockResolvedValue([]);
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

async function type(label: string, value: string) {
  await fireEvent.input(field(label), { target: { value } });
}

/** Coffre `perso` ouvert, avec le menu de gestion déplié. */
async function manage() {
  const props = { onLocked: vi.fn(), onExpired: vi.fn(), onDeleted: vi.fn() };
  render(OpenVault, { vault: { name: "perso", entries: 0 }, ...props });
  await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
  return props;
}

describe("changing the master password", () => {
  it("checks the new password, sends both and clears the fields", async () => {
    vi.mocked(changePassword).mockResolvedValue();
    await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Changer le mot de passe maître" }));
    await type("Mot de passe maître actuel", "maître");
    await type("Nouveau mot de passe maître", "nouveau");
    await type("Confirmation", "nouvau");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    await type("Confirmation", "nouveau");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Mot de passe maître changé.");
    expect(changePassword).toHaveBeenCalledWith("maître", "nouveau");
    expect(field("Mot de passe maître actuel").value).toBe("");
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("reports a wrong current password, or a lock in the meantime", async () => {
    vi.mocked(changePassword).mockRejectedValueOnce({ code: "wrongPassword", detail: null });
    const { onExpired } = await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Changer le mot de passe maître" }));
    const fill = async () => {
      await type("Mot de passe maître actuel", "faux");
      await type("Nouveau mot de passe maître", "nouveau");
      await type("Confirmation", "nouveau");
      await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    };
    await fill();
    expect(await screen.findByRole("alert")).toHaveTextContent("Mot de passe maître incorrect.");
    vi.mocked(changePassword).mockRejectedValueOnce({ code: "vaultLocked", detail: null });
    await fill();
    await vi.waitFor(() => {
      expect(onExpired).toHaveBeenCalled();
    });
  });
});

describe("importing a CSV file", () => {
  it("imports the chosen file and refreshes the list", async () => {
    vi.mocked(pickCsvFile).mockResolvedValueOnce(null).mockResolvedValueOnce("C:/export.csv");
    vi.mocked(importEntries).mockResolvedValue(2);
    await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Importer un CSV" }));
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(importEntries).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "2 entrées importées. Supprimez maintenant le fichier CSV",
    );
    expect(importEntries).toHaveBeenCalledWith("C:/export.csv");
    await vi.waitFor(() => {
      expect(listEntries).toHaveBeenCalledTimes(2);
    });
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.queryByRole("button", { name: "Choisir le fichier" })).not.toBeInTheDocument();
  });

  it("reports an invalid file, or a lock in the meantime", async () => {
    vi.mocked(pickCsvFile).mockResolvedValue("C:/a.csv");
    vi.mocked(importEntries)
      .mockRejectedValueOnce({ code: "csvUnknownKind", detail: "4" })
      .mockResolvedValueOnce(1)
      .mockRejectedValueOnce({ code: "vaultLocked", detail: null });
    const { onExpired } = await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Importer un CSV" }));
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("CSV, ligne 4 : type inconnu");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(await screen.findByRole("status")).toHaveTextContent("1 entrée importée.");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    await vi.waitFor(() => {
      expect(onExpired).toHaveBeenCalled();
    });
  });
});

describe("deleting the vault", () => {
  it("needs its name retyped", async () => {
    vi.mocked(deleteVault).mockRejectedValueOnce({ code: "ioError", detail: "occupé" }).mockResolvedValue();
    const { onDeleted } = await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer ce coffre" }));
    const confirm = () => screen.getByRole("button", { name: "Supprimer définitivement" });
    expect(screen.getByText(/Pour confirmer, retapez son nom\./)).toBeInTheDocument();
    expect(confirm()).toBeDisabled();
    await type("Nom du coffre", "perso");
    await fireEvent.click(confirm());
    expect(await screen.findByRole("alert")).toHaveTextContent("Erreur de fichier : occupé");
    await fireEvent.click(confirm());
    await vi.waitFor(() => {
      expect(onDeleted).toHaveBeenCalled();
    });
    expect(deleteVault).toHaveBeenCalledWith("perso", "perso");
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("closes the management menu on demand", async () => {
    await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    expect(screen.queryByRole("button", { name: "Importer un CSV" })).not.toBeInTheDocument();
  });
});

describe("backing up the vaults", () => {
  it("copies them to the chosen folder", async () => {
    vi.mocked(pickFolder).mockResolvedValueOnce(null).mockResolvedValueOnce("E:/");
    vi.mocked(backupVaults)
      .mockResolvedValueOnce({ folder: "E:/pswd-sauvegardes/2026", count: 1 })
      .mockResolvedValueOnce({ folder: "E:/pswd-sauvegardes/2027", count: 2 })
      .mockRejectedValueOnce({ code: "ioError", detail: "clé débranchée" });
    render(VaultList, { onOpen: vi.fn(), onCreate: vi.fn(), onRestore: vi.fn(), onSettings: vi.fn() });
    const backup = await screen.findByRole("button", { name: "Sauvegarder les coffres" });
    await fireEvent.click(backup);
    expect(backupVaults).not.toHaveBeenCalled();
    vi.mocked(pickFolder).mockResolvedValue("E:/");
    await fireEvent.click(backup);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "1 coffre sauvegardé dans E:/pswd-sauvegardes/2026.",
    );
    await fireEvent.click(backup);
    expect(await screen.findByText("2 coffres sauvegardés dans E:/pswd-sauvegardes/2027.")).toBeInTheDocument();
    await fireEvent.click(backup);
    expect(await screen.findByRole("alert")).toHaveTextContent("Erreur de fichier : clé débranchée");
    expect(backupVaults).toHaveBeenCalledWith("E:/");
  });

  it("is offered only when there is a vault", async () => {
    vi.mocked(listVaults).mockResolvedValue([]);
    const onRestore = vi.fn();
    const onSettings = vi.fn();
    render(VaultList, { onOpen: vi.fn(), onCreate: vi.fn(), onRestore, onSettings });
    await fireEvent.click(await screen.findByRole("button", { name: "Restaurer une sauvegarde" }));
    await fireEvent.click(screen.getByRole("button", { name: "Réglages" }));
    expect(screen.queryByRole("button", { name: "Sauvegarder les coffres" })).not.toBeInTheDocument();
    expect(onRestore).toHaveBeenCalled();
    expect(onSettings).toHaveBeenCalled();
  });
});

describe("restoring a vault", () => {
  it("restores under the same name when it is free", async () => {
    vi.mocked(pickFolder).mockResolvedValueOnce(null).mockResolvedValue("E:/s/2026-10-23_14-30-00");
    vi.mocked(backupContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreVault).mockResolvedValue("travail");
    const onRestored = vi.fn();
    render(Restore, { onRestored, onCancel: vi.fn() });
    const choose = screen.getByRole("button", { name: "Choisir la sauvegarde" });
    await fireEvent.click(choose);
    expect(backupContents).not.toHaveBeenCalled();
    await fireEvent.click(choose);
    expect(await screen.findByText("E:/s/2026-10-23_14-30-00")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "travail" }));
    await vi.waitFor(() => {
      expect(onRestored).toHaveBeenCalledWith("Coffre « travail » restauré.");
    });
  });

  it("reports a folder that is not a backup, or a failed restore", async () => {
    vi.mocked(pickFolder).mockResolvedValue("E:/s");
    vi.mocked(backupContents)
      .mockRejectedValueOnce({ code: "emptyBackup", detail: null })
      .mockResolvedValue(["perso"]);
    vi.mocked(restoreVault).mockRejectedValue({ code: "invalidBackup", detail: "altérée" });
    render(Restore, { onRestored: vi.fn(), onCancel: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Choisir la sauvegarde" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ce dossier ne contient aucun coffre sauvegardé.",
    );
    await fireEvent.click(screen.getByRole("button", { name: "Choisir la sauvegarde" }));
    await fireEvent.click(await screen.findByRole("button", { name: "perso" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Sauvegarde inutilisable : altérée");
  });
});

describe("settings", () => {
  it("reports settings out of bounds, and an unreadable configuration", async () => {
    vi.mocked(getSettings).mockResolvedValueOnce({
      autoLockMinutes: 10,
      clipboardClearSeconds: 20,
      backgroundLockSeconds: 60,
      theme: "auto",
    });
    vi.mocked(saveSettings).mockRejectedValue({ code: "invalidSettings", detail: null });
    const onClose = vi.fn();
    const { unmount } = render(Settings, { onClose });
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    await fireEvent.click(await screen.findByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Réglages hors des valeurs permises.",
    );
    unmount();
    vi.mocked(getSettings).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    render(Settings, { onClose });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalled();
  });
});

describe("forgotten master password", () => {
  it("sets a new one with the recovery key", async () => {
    vi.mocked(recoverVault).mockResolvedValue({ name: "perso", entries: 3 });
    const onUnlocked = vi.fn();
    render(Unlock, { name: "perso", onUnlocked, onCancel: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Mot de passe oublié ?" }));
    expect(screen.getByRole("heading")).toHaveTextContent("Récupérer « perso »");
    await type("Clé de récupération", "AAAA-BBBB");
    await type("Nouveau mot de passe maître", "nouveau");
    await type("Confirmation", "autre");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le nouveau mot de passe" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    await type("Confirmation", "nouveau");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le nouveau mot de passe" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 3 });
    });
    expect(recoverVault).toHaveBeenCalledWith("perso", "AAAA-BBBB", "nouveau");
    expect(field("Clé de récupération").value).toBe("");
  });

  it("explains a wrong key and can go back to the password", async () => {
    vi.mocked(recoverVault).mockRejectedValue({ code: "wrongRecoveryKey", detail: null });
    vi.mocked(unlockVault).mockResolvedValue({ name: "perso", entries: 0 });
    const onUnlocked = vi.fn();
    render(Unlock, { name: "perso", onUnlocked, onCancel: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Mot de passe oublié ?" }));
    await type("Clé de récupération", "faux");
    await type("Nouveau mot de passe maître", "n");
    await type("Confirmation", "n");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le nouveau mot de passe" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Clé de récupération incorrecte.");
    await fireEvent.click(screen.getByRole("button", { name: "Mot de passe maître" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("heading")).toHaveTextContent("Déverrouiller « perso »");
    await type("Mot de passe maître", "maître");
    await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalled();
    });
  });
});
