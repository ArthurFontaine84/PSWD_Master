import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import {
  createVault,
  currentVault,
  listVaults,
  lockVault,
  onVaultLocked,
  unlockVault,
} from "./lib/api";

vi.mock("./lib/api", () => ({
  listVaults: vi.fn(),
  createVault: vi.fn(),
  unlockVault: vi.fn(),
  lockVault: vi.fn(),
  currentVault: vi.fn(),
  onVaultLocked: vi.fn(),
}));

/** Le gestionnaire passé à `onVaultLocked`, et la fonction qui arrête l'écoute. */
let lockHandler: () => void = () => undefined;
const stopListening = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listVaults).mockResolvedValue(["perso"]);
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
    await fireEvent.click(screen.getByRole("button", { name: "Nouveau coffre" }));
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
    await fireEvent.click(screen.getByRole("button", { name: "Nouveau coffre" }));
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
});
