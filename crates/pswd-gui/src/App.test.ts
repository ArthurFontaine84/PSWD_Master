import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import {
  backupContents,
  createVault,
  currentVault,
  deleteVault,
  getSettings,
  listEntries,
  listTags,
  listVaults,
  lockVault,
  onVaultLocked,
  restoreVault,
  saveSettings,
  unlockVault,
  type Settings,
} from "./lib/api";
import { pickFolder } from "./lib/files";

vi.mock("./lib/api", () => ({
  listVaults: vi.fn(),
  createVault: vi.fn(),
  unlockVault: vi.fn(),
  lockVault: vi.fn(),
  currentVault: vi.fn(),
  onVaultLocked: vi.fn(),
  listEntries: vi.fn(),
  listTags: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  backupContents: vi.fn(),
  restoreVault: vi.fn(),
  deleteVault: vi.fn(),
}));
vi.mock("./lib/files", () => ({ pickFolder: vi.fn(), pickCsvFile: vi.fn() }));

const defaults: Settings = {
  autoLockMinutes: 10,
  clipboardClearSeconds: 20,
  backgroundLockSeconds: 60,
  theme: "light",
};

/** Le gestionnaire passé à `onVaultLocked`, et la fonction qui arrête l'écoute. */
let lockHandler: () => void = () => undefined;
const stopListening = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listVaults).mockResolvedValue(["perso"]);
  vi.mocked(listTags).mockResolvedValue([]);
  vi.mocked(listEntries).mockResolvedValue([
    { id: "1", kind: "note", title: "Wi-Fi", username: null, tags: [], hasPassword: false },
    { id: "2", kind: "note", title: "Code", username: null, tags: [], hasPassword: false },
  ]);
  vi.mocked(getSettings).mockResolvedValue(defaults);
  vi.mocked(onVaultLocked).mockImplementation((handler) => {
    lockHandler = handler;
    return Promise.resolve(stopListening);
  });
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

async function unlockPerso() {
  vi.mocked(unlockVault).mockResolvedValue({ name: "perso", entries: 2 });
  await fireEvent.click(await screen.findByRole("button", { name: "perso" }));
  await fireEvent.input(field("Mot de passe maître"), { target: { value: "maître" } });
  await fireEvent.click(screen.getByRole("button", { name: "Déverrouiller" }));
  await screen.findByText("2 entrées");
}

describe("App", () => {
  it("creates a vault, shows its recovery key once, then opens it", async () => {
    vi.mocked(createVault).mockResolvedValue({
      vault: { name: "neuf", entries: 0 },
      recoveryKey: "AAAA-BBBB",
    });
    render(App);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("PSWD Master");
    await fireEvent.click(await screen.findByRole("button", { name: "Nouveau coffre" }));
    await fireEvent.input(field("Nom du coffre"), { target: { value: "neuf" } });
    await fireEvent.input(field("Mot de passe maître"), { target: { value: "maître" } });
    await fireEvent.input(field("Confirmation"), { target: { value: "maître" } });
    await fireEvent.click(screen.getByRole("button", { name: "Créer" }));
    expect(await screen.findByText("AAAA-BBBB")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("checkbox"));
    await fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Coffre « neuf »");
    expect(screen.queryByText("AAAA-BBBB")).not.toBeInTheDocument();
  });

  it("goes back to the list when creating or unlocking is cancelled", async () => {
    render(App);
    await fireEvent.click(await screen.findByRole("button", { name: "Nouveau coffre" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await fireEvent.click(await screen.findByRole("button", { name: "perso" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
  });

  it("unlocks a vault and locks it again", async () => {
    vi.mocked(lockVault).mockResolvedValue();
    render(App);
    await unlockPerso();
    await fireEvent.click(screen.getByRole("button", { name: "Verrouiller" }));
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("returns to the list when the vault locks itself", async () => {
    render(App);
    await unlockPerso();
    lockHandler();
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Coffre verrouillé après une période d'inactivité.",
    );
  });

  it("reports activity while a vault is open, and notices a missed lock", async () => {
    vi.mocked(currentVault).mockResolvedValue(null);
    render(App);
    await fireEvent.keyDown(window, { key: "a" });
    expect(currentVault).not.toHaveBeenCalled();
    await unlockPerso();
    await fireEvent.pointerDown(window);
    expect(currentVault).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Coffre verrouillé après une période d'inactivité.",
    );
  });

  it("keeps the vault open when the backend still has it", async () => {
    vi.mocked(currentVault).mockResolvedValue({ name: "perso", entries: 2 });
    render(App);
    await unlockPerso();
    await fireEvent.keyDown(window, { key: "a" });
    await vi.waitFor(() => {
      expect(currentVault).toHaveBeenCalled();
    });
    expect(screen.getByText("2 entrées")).toBeInTheDocument();
  });

  it("stops listening when closed", async () => {
    const { unmount } = render(App);
    unmount();
    await vi.waitFor(() => {
      expect(stopListening).toHaveBeenCalled();
    });
  });

  it("applies the saved theme at startup, or keeps the system one", async () => {
    render(App);
    await vi.waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("light");
    });
    document.documentElement.removeAttribute("data-theme");
    vi.mocked(getSettings).mockRejectedValue({ code: "configInvalid", detail: "x" });
    render(App);
    await screen.findAllByRole("button", { name: "perso" });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("changes the settings and applies the new theme", async () => {
    vi.mocked(saveSettings).mockImplementation((settings) => Promise.resolve(settings));
    render(App);
    await fireEvent.click(screen.getByRole("button", { name: "Réglages" }));
    const theme = await screen.findByRole("combobox", { name: "Thème" });
    await fireEvent.change(theme, { target: { value: "dark" } });
    await fireEvent.input(field("Verrouillage après inactivité"), { target: { value: "5" } });
    await fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Réglages enregistrés.");
    expect(saveSettings).toHaveBeenCalledWith({ ...defaults, autoLockMinutes: 5, theme: "dark" });
    expect(document.documentElement.dataset.theme).toBe("dark");
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
  });

  it("restores a vault and says so in the list", async () => {
    vi.mocked(pickFolder).mockResolvedValue("E:/pswd-sauvegardes/2026-10-23_14-30-00");
    vi.mocked(backupContents).mockResolvedValue(["perso"]);
    vi.mocked(restoreVault).mockResolvedValue("perso-sauvegarde-2026-10-23");
    render(App);
    await fireEvent.click(await screen.findByRole("button", { name: "Restaurer une sauvegarde" }));
    await fireEvent.click(screen.getByRole("button", { name: "Choisir la sauvegarde" }));
    await fireEvent.click(await screen.findByRole("button", { name: "perso" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Coffre « perso » restauré sous le nom « perso-sauvegarde-2026-10-23 »",
    );
    expect(restoreVault).toHaveBeenCalledWith("E:/pswd-sauvegardes/2026-10-23_14-30-00", "perso");
  });

  it("leaves the restore screen when cancelled", async () => {
    render(App);
    await fireEvent.click(await screen.findByRole("button", { name: "Restaurer une sauvegarde" }));
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(await screen.findByRole("button", { name: "perso" })).toBeInTheDocument();
  });

  it("goes back to the list once the open vault is deleted", async () => {
    vi.mocked(deleteVault).mockResolvedValue();
    render(App);
    await unlockPerso();
    await fireEvent.click(screen.getByRole("button", { name: "Gérer le coffre" }));
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer ce coffre" }));
    await fireEvent.input(field("Nom du coffre"), { target: { value: "perso" } });
    vi.mocked(listVaults).mockResolvedValue([]);
    await fireEvent.click(screen.getByRole("button", { name: "Supprimer définitivement" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Coffre « perso » supprimé.");
    expect(deleteVault).toHaveBeenCalledWith("perso", "perso");
  });
});
