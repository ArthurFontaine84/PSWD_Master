import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  accountStatus,
  addDeviceKey,
  connectServer,
  disconnectServer,
  serverStatus,
  syncServer,
  type Exchange,
  type ServerStatus,
} from "../lib/api";
import Toast from "../components/Toast.svelte";
import Server from "./Server.svelte";

vi.mock("../lib/api", () => ({
  accountStatus: vi.fn(),
  serverStatus: vi.fn(),
  connectServer: vi.fn(),
  syncServer: vi.fn(),
  addDeviceKey: vi.fn(),
  disconnectServer: vi.fn(),
}));

const disconnected: ServerStatus = { address: null, vaults: [] };
const connected: ServerStatus = { address: "pswd.local:8443", vaults: [] };
const nothing: Exchange = { removed: [], downloaded: [], notDownloaded: [], published: [], notPublished: [] };

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(accountStatus).mockResolvedValue({ accountName: "Camille", isAdmin: false, publicAddress: null });
  render(Toast);
});

function field(label: string): HTMLInputElement {
  return screen.getByLabelText(label, { exact: false });
}

function show(status: ServerStatus) {
  vi.mocked(serverStatus).mockResolvedValue(status);
  const onClose = vi.fn();
  render(Server, { onClose, onAdmin: vi.fn() });
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
    render(Server, { onClose, onAdmin: vi.fn() });
    expect(await screen.findByRole("alert")).toHaveTextContent("Configuration invalide : ligne 2");
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalled();
  });
});

describe("Server, once connected", () => {
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
    expect(await screen.findByText("Aucun coffre à échanger avec le serveur.")).toBeInTheDocument();
    vi.mocked(syncServer).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Échange avec le serveur impossible : serveur injoignable.",
    );
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
    expect(screen.getByText(/Seuls les coffres identiques/)).toBeInTheDocument();
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
        "Appareil déconnecté du serveur. Coffre « perso » retiré de cet appareil ; il reste sur le serveur.",
      ),
    ).toBeInTheDocument();
    expect(await screen.findByText(/Connectez cet appareil/)).toBeInTheDocument();
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

  it("offers the administration panel to an administrator only", async () => {
    vi.mocked(accountStatus).mockResolvedValue({ accountName: "Arthur", isAdmin: true, publicAddress: "maison:8443" });
    vi.mocked(serverStatus).mockResolvedValue(connected);
    const onAdmin = vi.fn();
    const { unmount } = render(Server, { onClose: vi.fn(), onAdmin });
    expect(await screen.findByText("Compte : Arthur (administrateur).")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Administration" }));
    expect(onAdmin).toHaveBeenCalledWith("pswd.local:8443", "maison:8443");
    unmount();

    vi.mocked(accountStatus).mockResolvedValue({ accountName: "Camille", isAdmin: false, publicAddress: null });
    show(connected);
    expect(await screen.findByText("Compte : Camille.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Administration" })).not.toBeInTheDocument();
  });

  it("stays usable when the server does not answer", async () => {
    vi.mocked(accountStatus).mockRejectedValue({ code: "serverError", detail: "serveur injoignable" });
    show(connected);
    expect(await screen.findByText("Connecté au serveur pswd.local:8443.")).toBeInTheDocument();
    expect(screen.queryByText(/Compte :/)).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
