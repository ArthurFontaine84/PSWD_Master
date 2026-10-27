import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  backupFileContents,
  backupFileName,
  backupToFile,
  changePassword,
  deleteVault,
  getSettings,
  importEntries,
  listEntries,
  listTags,
  listVaults,
  lockVault,
  recoverVault,
  restoreFromFile,
  saveSettings,
} from "../lib/api";
import { pickBackupDestination, pickBackupFile, pickCsvFile } from "../lib/files";
import Toast from "../components/Toast.svelte";
import { goBack } from "../lib/back";
import Backup from "./Backup.svelte";
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
  backupFileName: vi.fn(),
  backupToFile: vi.fn(),
  backupFileContents: vi.fn(),
  restoreFromFile: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
}));
vi.mock("../lib/files", () => ({
  pickCsvFile: vi.fn(),
  pickBackupFile: vi.fn(),
  pickBackupDestination: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  render(Toast);
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

function expectVerbatim(element: HTMLElement) {
  expect(element).toHaveAttribute("autocapitalize", "off");
  expect(element).toHaveAttribute("autocorrect", "off");
  expect(element).toHaveAttribute("spellcheck", "false");
}

/** Coffre `perso` ouvert, avec le menu de gestion déplié. */
async function manage() {
  const props = { onLocked: vi.fn(), onExpired: vi.fn(), onDeleted: vi.fn() };
  render(OpenVault, { vault: { name: "perso", entries: 0 }, ...props });
  await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
  return props;
}

describe("changing the master password", () => {
  it("checks the new password, sends both, then closes with a confirmation", async () => {
    vi.mocked(changePassword).mockResolvedValue();
    await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Changer le mot de passe maître" }));
    await type("Mot de passe maître actuel", "maître");
    await type("Nouveau mot de passe maître", "Nouveau-2026!");
    await type("Confirmation", "Nouvau-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    await type("Confirmation", "Nouveau-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Mot de passe maître changé.");
    expect(changePassword).toHaveBeenCalledWith("maître", "Nouveau-2026!");
    expect(screen.queryByLabelText("Mot de passe maître actuel", { exact: false })).not.toBeInTheDocument();
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("reports a wrong current password, or a lock in the meantime", async () => {
    vi.mocked(changePassword).mockRejectedValueOnce({ code: "wrongPassword", detail: null });
    const { onExpired } = await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Changer le mot de passe maître" }));
    const fill = async () => {
      await type("Mot de passe maître actuel", "faux");
      await type("Nouveau mot de passe maître", "Nouveau-2026!");
      await type("Confirmation", "Nouveau-2026!");
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
  it("imports the chosen file, refreshes the list and closes", async () => {
    vi.mocked(pickCsvFile).mockResolvedValueOnce(null).mockResolvedValueOnce("C:/export.csv");
    vi.mocked(importEntries).mockResolvedValue(2);
    await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Importer un CSV" }));
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
    expect(screen.queryByRole("button", { name: "Choisir le fichier" })).not.toBeInTheDocument();
  });

  it("reports an invalid file, or a lock in the meantime", async () => {
    vi.mocked(pickCsvFile).mockResolvedValue("C:/a.csv");
    vi.mocked(importEntries)
      .mockRejectedValueOnce({ code: "csvUnknownKind", detail: "4" })
      .mockResolvedValueOnce(1)
      .mockRejectedValueOnce({ code: "vaultLocked", detail: null });
    const { onExpired } = await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Importer un CSV" }));
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("CSV, ligne 4 : type inconnu");
    await fireEvent.click(screen.getByRole("button", { name: "Choisir le fichier" }));
    expect(await screen.findByRole("status")).toHaveTextContent("1 entrée importée.");
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Importer un CSV" }));
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
    await fireEvent.click(screen.getByRole("menuitem", { name: "Supprimer ce coffre" }));
    const confirm = () => screen.getByRole("button", { name: "Supprimer" });
    expect(screen.getByText(/Pour confirmer, retapez son nom\./)).toBeInTheDocument();
    expect(confirm()).toBeDisabled();
    expectVerbatim(field("Nom du coffre"));
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

  it("is closed by the back button", async () => {
    await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Supprimer ce coffre" }));
    goBack();
    expect(await screen.findByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("closes the management menu on demand", async () => {
    await manage();
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the management menu once an action is chosen", async () => {
    await manage();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Importer un CSV" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("Importer un CSV");
  });

  it("closes the management menu on a click outside, Escape or the back button", async () => {
    await manage();
    const button = screen.getByRole("button", { name: "Gérer le coffre" });
    await fireEvent.click(screen.getByRole("menu"));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("searchbox"));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    expect(button).toHaveAttribute("aria-expanded", "false");
    await fireEvent.click(button);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await fireEvent.click(button);
    goBack();
    await vi.waitFor(() => {
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });
    expect(lockVault).not.toHaveBeenCalled();
  });
});

describe("backing up the vaults", () => {
  function render_(onBackedUp = vi.fn()) {
    render(Backup, { onBackedUp, onCancel: vi.fn() });
    return onBackedUp;
  }

  it("is opened from the list, only when there is a vault", async () => {
    const onBackup = vi.fn();
    const { unmount } = render(VaultList, { onOpen: vi.fn(), onBackup, onRestore: vi.fn() });
    await fireEvent.click(await screen.findByRole("button", { name: "Sauvegarder" }));
    expect(onBackup).toHaveBeenCalled();
    unmount();
    vi.mocked(listVaults).mockResolvedValue([]);
    const onRestore = vi.fn();
    render(VaultList, { onOpen: vi.fn(), onBackup, onRestore });
    await screen.findByText("Aucun coffre sur cet appareil.");
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer" }));
    expect(screen.queryByRole("button", { name: "Sauvegarder" })).not.toBeInTheDocument();
    expect(onRestore).toHaveBeenCalled();
  });

  it("lists the vaults of the device, none of them checked", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    render_();
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    await screen.findByRole("group", { name: "Coffres à sauvegarder" });
    expect(screen.getByRole("checkbox", { name: "perso" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "travail" })).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Sauvegarder le coffre" })).toBeDisabled();
  });

  it("writes the checked vaults only to the chosen .pswd file", async () => {
    vi.mocked(listVaults).mockResolvedValue(["famille", "perso", "travail"]);
    vi.mocked(backupFileName).mockResolvedValue("pswd-sauvegarde_2026-10-24_11-20-58.pswd");
    vi.mocked(pickBackupDestination).mockResolvedValueOnce(null).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupToFile).mockResolvedValue(2);
    const onBackedUp = render_();
    await fireEvent.click(await screen.findByRole("checkbox", { name: "travail" }));
    await fireEvent.click(screen.getByRole("checkbox", { name: "famille" }));
    const confirm = screen.getByRole("button", { name: "Sauvegarder les 2 coffres" });
    await fireEvent.click(confirm);
    await vi.waitFor(() => {
      expect(pickBackupDestination).toHaveBeenCalledWith(
        "Enregistrer la sauvegarde des coffres",
        "pswd-sauvegarde_2026-10-24_11-20-58.pswd",
      );
    });
    await vi.waitFor(() => {
      expect(confirm).toBeEnabled();
    });
    expect(backupToFile).not.toHaveBeenCalled();
    await fireEvent.click(confirm);
    await vi.waitFor(() => {
      expect(onBackedUp).toHaveBeenCalledWith("2 coffres sauvegardés.");
    });
    expect(backupToFile).toHaveBeenCalledWith("D:/s.pswd", ["famille", "travail"]);
  });

  it("selects everything at once", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    vi.mocked(pickBackupDestination).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupToFile).mockResolvedValue(2);
    render_();
    await fireEvent.click(await screen.findByRole("checkbox", { name: "Tout sélectionner" }));
    await fireEvent.click(screen.getByRole("button", { name: "Sauvegarder les 2 coffres" }));
    await vi.waitFor(() => {
      expect(backupToFile).toHaveBeenCalledWith("D:/s.pswd", ["perso", "travail"]);
    });
  });

  it("shows the writing errors and lets the user try again", async () => {
    vi.mocked(pickBackupDestination).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupToFile)
      .mockRejectedValueOnce({ code: "fileUnwritable", detail: "accès refusé" })
      .mockResolvedValue(1);
    const onBackedUp = render_();
    await fireEvent.click(await screen.findByRole("checkbox", { name: "perso" }));
    const confirm = screen.getByRole("button", { name: "Sauvegarder le coffre" });
    await fireEvent.click(confirm);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Écriture du fichier impossible : accès refusé",
    );
    expect(onBackedUp).not.toHaveBeenCalled();
    await fireEvent.click(confirm);
    await vi.waitFor(() => {
      expect(onBackedUp).toHaveBeenCalledWith("1 coffre sauvegardé.");
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says when the vaults cannot be listed", async () => {
    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    const onCancel = vi.fn();
    render(Backup, { onBackedUp: vi.fn(), onCancel });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onCancel).toHaveBeenCalled();
  });
});

describe("restoring vaults", () => {
  function render_(onRestored = vi.fn()) {
    render(Restore, { onRestored, onCancel: vi.fn() });
    return onRestored;
  }

  async function chooseFile() {
    await fireEvent.click(screen.getByRole("button", { name: "fichier .pswd" }));
    return screen.findByRole("group", { name: "Coffres à restaurer" });
  }

  it("lists the vaults of the chosen .pswd file, none of them checked", async () => {
    vi.mocked(pickBackupFile).mockResolvedValueOnce(null).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents)
      .mockRejectedValueOnce({ code: "invalidBackup", detail: "fichier de sauvegarde mal formé" })
      .mockResolvedValue(["perso", "travail"]);
    render_();
    const choose = screen.getByRole("button", { name: "fichier .pswd" });
    await fireEvent.click(choose);
    expect(backupFileContents).not.toHaveBeenCalled();
    await fireEvent.click(choose);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Sauvegarde inutilisable : fichier de sauvegarde mal formé",
    );
    await fireEvent.click(choose);
    expect(await screen.findByText("s.pswd")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "perso" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "travail" })).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Restaurer le coffre sélectionné" })).toBeDisabled();
  });

  it("restores the checked vault only", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreFromFile).mockResolvedValue("perso-sauvegarde-2026-10-24");
    const onRestored = render_();
    await chooseFile();
    await fireEvent.click(screen.getByRole("checkbox", { name: "perso" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer le coffre sélectionné" }));
    await vi.waitFor(() => {
      expect(onRestored).toHaveBeenCalledWith(
        "Coffre « perso » restauré sous le nom « perso-sauvegarde-2026-10-24 » : ce nom était déjà pris.",
      );
    });
    expect(restoreFromFile).toHaveBeenCalledTimes(1);
    expect(restoreFromFile).toHaveBeenCalledWith("D:/s.pswd", "perso");
  });

  it("selects everything at once, and restores every vault", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreFromFile).mockImplementation((_, name) =>
      Promise.resolve(name === "perso" ? "perso-sauvegarde-2026-10-24" : name),
    );
    const onRestored = render_();
    await chooseFile();
    const all = screen.getByRole("checkbox", { name: "Tout sélectionner" });
    await fireEvent.click(all);
    expect(screen.getByRole("checkbox", { name: "perso" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "travail" })).toBeChecked();
    expect(all).toBeChecked();
    await fireEvent.click(all);
    expect(screen.getByRole("checkbox", { name: "perso" })).not.toBeChecked();
    await fireEvent.click(all);
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer les 2 coffres sélectionnés" }));
    await vi.waitFor(() => {
      expect(onRestored).toHaveBeenCalledWith(
        "2 coffres restaurés (renommés : « perso » → « perso-sauvegarde-2026-10-24 »).",
      );
    });
    expect(restoreFromFile).toHaveBeenCalledWith("D:/s.pswd", "travail");
  });

  it("shows no address when it does not give the file name", async () => {
    const address = "content://com.android.providers.downloads.documents/document/msf%3A1000";
    vi.mocked(pickBackupFile).mockResolvedValue(address);
    vi.mocked(backupFileContents).mockResolvedValue(["perso"]);
    render_();
    await chooseFile();
    const panel = screen.getByRole("region", { name: "Restaurer une sauvegarde" });
    expect(panel).not.toHaveTextContent(/content:|msf/);
  });

  it("says nothing about names when none had to change", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreFromFile).mockImplementation((_, name) => Promise.resolve(name));
    const onRestored = render_();
    await chooseFile();
    await fireEvent.click(screen.getByRole("checkbox", { name: "Tout sélectionner" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer les 2 coffres sélectionnés" }));
    await vi.waitFor(() => {
      expect(onRestored).toHaveBeenCalledWith("2 coffres restaurés.");
    });
  });

  it("restores under the same name when it is free", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreFromFile).mockResolvedValue("travail");
    const onRestored = render_();
    await chooseFile();
    await fireEvent.click(screen.getByRole("checkbox", { name: "travail" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer le coffre sélectionné" }));
    await vi.waitFor(() => {
      expect(onRestored).toHaveBeenCalledWith("Coffre « travail » restauré.");
    });
  });

  it("drops the vaults already restored when one fails", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockResolvedValue(["perso", "travail"]);
    vi.mocked(restoreFromFile)
      .mockResolvedValueOnce("perso")
      .mockRejectedValueOnce({ code: "invalidBackup", detail: "altérée" });
    const onRestored = render_();
    await chooseFile();
    await fireEvent.click(screen.getByRole("checkbox", { name: "Tout sélectionner" }));
    await fireEvent.click(screen.getByRole("button", { name: "Restaurer les 2 coffres sélectionnés" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Sauvegarde inutilisable : altérée");
    expect(screen.queryByRole("checkbox", { name: "perso" })).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "travail" })).toBeChecked();
    expect(screen.getByRole("button", { name: "Restaurer le coffre sélectionné" })).toBeEnabled();
    expect(onRestored).not.toHaveBeenCalled();
  });

  it("reports a file without vaults", async () => {
    vi.mocked(pickBackupFile).mockResolvedValue("D:/s.pswd");
    vi.mocked(backupFileContents).mockRejectedValue({ code: "emptyBackup", detail: null });
    render_();
    await fireEvent.click(screen.getByRole("button", { name: "fichier .pswd" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Cette sauvegarde ne contient aucun coffre.",
    );
  });

  it("offers no folder, even on a computer", () => {
    render_();
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Annuler",
      "fichier .pswd",
    ]);
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
    expectVerbatim(field("Clé de récupération"));
    await type("Clé de récupération", "AAAA-BBBB");
    await type("Nouveau mot de passe maître", "Nouveau-2026!");
    await type("Confirmation", "autre");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Les deux saisies ne correspondent pas.");
    await type("Confirmation", "Nouveau-2026!");
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 3 });
    });
    expect(recoverVault).toHaveBeenCalledWith("perso", "AAAA-BBBB", "Nouveau-2026!");
    expect(field("Clé de récupération").value).toBe("");
  });

  it("explains a wrong key, then lets the key be typed again", async () => {
    vi.mocked(recoverVault)
      .mockRejectedValueOnce({ code: "wrongRecoveryKey", detail: null })
      .mockResolvedValue({ name: "perso", entries: 0 });
    const onUnlocked = vi.fn();
    render(Unlock, { name: "perso", onUnlocked, onCancel: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: "Mot de passe oublié ?" }));
    const buttons = screen.getAllByRole("button").map((button) => button.getAttribute("aria-label") ?? button.textContent);
    expect(buttons).toEqual(["Afficher le mot de passe", "Afficher le mot de passe", "Annuler", "Enregistrer"]);
    const fill = async (key: string) => {
      await type("Clé de récupération", key);
      await type("Nouveau mot de passe maître", "Nouveau-2026!");
      await type("Confirmation", "Nouveau-2026!");
      await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    };
    await fill("faux");
    expect(await screen.findByRole("alert")).toHaveTextContent("Clé de récupération incorrecte.");
    expect(screen.getByRole("heading")).toHaveTextContent("Récupérer « perso »");
    await fill("bonne");
    await vi.waitFor(() => {
      expect(onUnlocked).toHaveBeenCalledWith({ name: "perso", entries: 0 });
    });
    expect(recoverVault).toHaveBeenLastCalledWith("perso", "bonne", "Nouveau-2026!");
  });
});
