import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  accountStatus,
  addDeviceKey,
  connectServer,
  disconnectServer,
  downloadAgain,
  ignoredVaults,
  refreshServer,
  serverStatus,
  syncServer,
  type AccountStatus,
  type Exchange,
  type ServerStatus,
} from "../lib/api";
import Toast from "../components/Toast.svelte";
import Server from "./Server.svelte";

vi.mock("../lib/api", () => ({
  accountStatus: vi.fn(),
  refreshServer: vi.fn(),
  serverStatus: vi.fn(),
  connectServer: vi.fn(),
  syncServer: vi.fn(),
  addDeviceKey: vi.fn(),
  disconnectServer: vi.fn(),
  ignoredVaults: vi.fn(),
  downloadAgain: vi.fn(),
}));

const disconnected: ServerStatus = { address: null, trouble: null, vaults: [] };
const connected: ServerStatus = { address: "pswd.local:8443", trouble: null, vaults: [] };
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

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(accountStatus).mockResolvedValue(account("Camille", false));
  vi.mocked(ignoredVaults).mockResolvedValue([]);
  render(Toast);
});

function account(accountName: string, isAdmin: boolean): AccountStatus {
  return { address: "pswd.local:8443", accountId: "a1", accountName, isAdmin, publicAddress: null };
}

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

function show(status: ServerStatus) {
  vi.mocked(serverStatus).mockResolvedValue(status);
  vi.mocked(refreshServer).mockImplementation(() => serverStatus());
  const onClose = vi.fn();
  render(Server, { onClose });
  return onClose;
}

async function fillConnection() {
  await screen.findByText(/Connectez cet appareil/);
  await fireEvent.input(field("Adresse du serveur"), { target: { value: "pswd.local" } });
  await fireEvent.input(field("Clé de connexion"), { target: { value: "AAAA-BBBB" } });
  await fireEvent.input(field("Nom de cet appareil"), { target: { value: "PC" } });
  await fireEvent.input(field("Nom du compte"), { target: { value: "Arthur" } });
}

describe("Server, before connecting", () => {
  it("checks the form before sending anything", async () => {
    show(disconnected);
    await screen.findByText(/Connectez cet appareil/);
    await fireEvent.click(screen.getByRole("button", { name: "Se connecter" }));
    expect(screen.getByRole("alert")).toHaveTextContent("L'adresse du serveur est obligatoire.");
    expect(connectServer).not.toHaveBeenCalled();
  });

  it("keeps the address and the key as typed", async () => {
    show(disconnected);
    await screen.findByText(/Connectez cet appareil/);
    for (const label of ["Adresse du serveur", "Clé de connexion"]) {
      expect(field(label)).toHaveAttribute("autocapitalize", "off");
    }
  });

  it("connects the device, then exchanges its vaults", async () => {
    show(disconnected);
    await fillConnection();
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncServer).mockResolvedValue({ ...nothing, published: ["perso"] });
    await fireEvent.click(screen.getByRole("button", { name: "Se connecter" }));
    expect(await screen.findByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
    expect(connectServer).toHaveBeenCalledWith("pswd.local", "AAAA-BBBB", "PC", "Arthur");
    expect(
      await screen.findByText("Appareil connecté au serveur. Coffre « perso » publié."),
    ).toBeInTheDocument();
  });

  it("forgets the key once sent, and shows a refusal", async () => {
    show(disconnected);
    await fillConnection();
    vi.mocked(connectServer).mockRejectedValue({ code: "invalidConnectionKey", detail: null });
    await fireEvent.click(screen.getByRole("button", { name: "Se connecter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Clé de connexion mal saisie");
    expect(field("Clé de connexion").value).toBe("");
    expect(syncServer).not.toHaveBeenCalled();
  });

  it("asks to check the address of a server that does not answer", async () => {
    show(disconnected);
    await fillConnection();
    vi.mocked(connectServer).mockRejectedValue({ code: "serverUnreachable", detail: null });
    await fireEvent.click(screen.getByRole("button", { name: "Se connecter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Serveur injoignable. Vérifiez l'adresse saisie.");
  });

  it("says when the first exchange is incomplete", async () => {
    show(disconnected);
    await fillConnection();
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncServer).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    await fireEvent.click(screen.getByRole("button", { name: "Se connecter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Échange avec le serveur incomplet. Échange avec le serveur impossible : serveur injoignable. Réessayez avec « Synchroniser ».",
    );
    expect(await screen.findByText("Appareil connecté au serveur.")).toBeInTheDocument();
  });

  it("explains an unreadable state and goes back", async () => {
    vi.mocked(serverStatus).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    const onClose = vi.fn();
    render(Server, { onClose });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalled();
  });
});

describe("Server, once connected", () => {
  it("lists the vaults removed from this device and downloads them again", async () => {
    vi.mocked(ignoredVaults)
      .mockResolvedValueOnce([{ id: "v1", name: "perso" }])
      .mockResolvedValue([]);
    vi.mocked(downloadAgain)
      .mockRejectedValueOnce({ code: "serverUnreachable", detail: null })
      .mockResolvedValue({ ...nothing, downloaded: [{ name: "perso", localName: "perso" }] });
    show(connected);
    expect(await screen.findByRole("heading", { name: "Coffres retirés de cet appareil" })).toBeInTheDocument();
    const again = screen.getByRole("button", { name: "Télécharger de nouveau « perso »" });
    expect(again).toHaveTextContent("Télécharger");
    await fireEvent.click(again);
    expect(await screen.findByRole("alert")).toHaveTextContent("Serveur injoignable.");
    await fireEvent.click(again);
    expect(await screen.findByText("Coffre « perso » téléchargé.")).toBeInTheDocument();
    expect(downloadAgain).toHaveBeenCalledWith("v1");
    await vi.waitFor(() => {
      expect(screen.queryByRole("heading", { name: "Coffres retirés de cet appareil" })).not.toBeInTheDocument();
    });
  });

  it("does not list the vaults removed from this device without the server", async () => {
    vi.mocked(ignoredVaults).mockRejectedValue({ code: "serverUnreachable", detail: null });
    show(connected);
    await vi.waitFor(() => {
      expect(ignoredVaults).toHaveBeenCalled();
    });
    expect(screen.queryByRole("heading", { name: "Coffres retirés de cet appareil" })).not.toBeInTheDocument();
    show({ ...connected, trouble: { since: 0, reason: "serveur injoignable" } });
    expect(await screen.findAllByText(/Échanges impossibles depuis/)).not.toHaveLength(0);
    expect(ignoredVaults).toHaveBeenCalledTimes(1);
  });

  it("synchronizes and reports each failure", async () => {
    show(connected);
    vi.mocked(syncServer).mockResolvedValue({
      ...nothing,
      notPublished: [{ name: "abime", reason: "refusé" }],
    });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Coffre « abime » non publié : refusé.");
  });

  it("says when there is nothing to exchange, or when the server cannot be reached", async () => {
    show(connected);
    vi.mocked(syncServer).mockResolvedValue(nothing);
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Coffres à jour.")).toBeInTheDocument();
    vi.mocked(syncServer).mockRejectedValue({ code: "serverUnreachable", detail: null });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/^Serveur injoignable\.$/);
  });

  it("shows the key of a new device until it is closed", async () => {
    show(connected);
    vi.mocked(addDeviceKey).mockResolvedValue("CCCC-DDDD");
    await fireEvent.click(await screen.findByRole("button", { name: "Ajouter un appareil" }));
    expect(await screen.findByText("CCCC-DDDD")).toBeInTheDocument();
    expect(screen.getByText(/connectez-vous au serveur pswd.local:8443 avec cette clé/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(screen.queryByText("CCCC-DDDD")).not.toBeInTheDocument();
    expect(screen.getByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
  });

  it("keeps the local copies unless told otherwise", async () => {
    show(connected);
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    const keep = screen.getByRole("checkbox", { name: "Garder une copie des coffres sur cet appareil" });
    expect(keep).toBeChecked();
    await fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.getByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
    expect(disconnectServer).not.toHaveBeenCalled();
  });

  it("disconnects and reports the vaults kept on the device", async () => {
    show(connected);
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    await fireEvent.click(screen.getByRole("checkbox"));
    expect(screen.getByText(/Seuls les coffres déjà à jour sur le serveur/)).toBeInTheDocument();
    vi.mocked(serverStatus).mockResolvedValue(disconnected);
    vi.mocked(disconnectServer).mockResolvedValue({
      revokeError: null,
      removed: ["perso"],
      kept: [{ name: "modifie", reason: null }],
    });
    await fireEvent.click(screen.getByRole("button", { name: "Se déconnecter" }));
    expect(disconnectServer).toHaveBeenCalledWith(false);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Coffre « modifie » gardé : des modifications ne sont pas encore sur le serveur.",
    );
    expect(
      await screen.findByText(
        "Appareil déconnecté du serveur. Coffre « perso » retiré de cet appareil ; il reste sur le serveur. Les coffres gardés sur cet appareil restent liés à ce compte : s'ils appartiennent à quelqu'un d'autre, supprimez-les avant de connecter un autre compte.",
      ),
    ).toBeInTheDocument();
    expect(await screen.findByText(/Connectez cet appareil/)).toBeInTheDocument();
  });

  it("reminds that the copies kept belong to this account, only if there are some", async () => {
    const perso = { name: "perso", state: "synced" as const, shared: false };
    const nobody = { revokeError: null, removed: [], kept: [] };
    vi.mocked(disconnectServer).mockResolvedValue(nobody);
    show({ ...connected, vaults: [perso] });
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    await fireEvent.click(screen.getByRole("button", { name: "Se déconnecter" }));
    expect(disconnectServer).toHaveBeenCalledWith(true);
    expect(await screen.findByText(/Appareil déconnecté du serveur\. Les coffres gardés/)).toBeInTheDocument();
    cleanup();
    render(Toast);

    show(connected);
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    await fireEvent.click(screen.getByRole("button", { name: "Se déconnecter" }));
    expect(await screen.findByText("Appareil déconnecté du serveur.")).toBeInTheDocument();
  });

  it("shows a failed disconnection", async () => {
    show(connected);
    await fireEvent.click(await screen.findByRole("button", { name: "Se déconnecter" }));
    vi.mocked(disconnectServer).mockRejectedValue({ code: "configUnreadable", detail: "accès refusé" });
    await fireEvent.click(screen.getByRole("button", { name: "Se déconnecter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration illisible : accès refusé");
  });

  it("goes back to the list", async () => {
    const onClose = show(connected);
    await fireEvent.click(await screen.findByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("shows the account of the device, and its role", async () => {
    vi.mocked(accountStatus).mockResolvedValue(account("Arthur", true));
    vi.mocked(serverStatus).mockResolvedValue(connected);
    const { unmount } = render(Server, { onClose: vi.fn() });
    expect(await screen.findByText("Compte : Arthur (administrateur).")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Administration" })).not.toBeInTheDocument();
    unmount();

    vi.mocked(accountStatus).mockResolvedValue(account("Camille", false));
    show(connected);
    expect(await screen.findByText("Compte : Camille.")).toBeInTheDocument();
  });

  it("stays usable when the server does not answer", async () => {
    vi.mocked(accountStatus).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    show(connected);
    expect(await screen.findByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
    expect(screen.queryByText(/Compte :/)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("tells a registered server from a connected one", async () => {
    const since = new Date(2026, 10, 6, 14, 32).getTime();
    show({ ...connected, trouble: { since, reason: "serveur injoignable" } });
    expect(await screen.findByText("Serveur enregistré : pswd.local:8443.")).toBeInTheDocument();
    expect(screen.getByText("Échanges impossibles depuis 14:32 : serveur injoignable.")).toHaveAttribute(
      "role",
      "status",
    );
    expect(screen.queryByText(/Connecté au serveur/)).not.toBeInTheDocument();
    expect(accountStatus).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Se déconnecter" })).toBeEnabled();

    vi.mocked(syncServer).mockResolvedValue(nothing);
    vi.mocked(serverStatus).mockResolvedValue(connected);
    await fireEvent.click(screen.getByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
    expect(screen.queryByText(/Échanges impossibles/)).not.toBeInTheDocument();
  });
});
