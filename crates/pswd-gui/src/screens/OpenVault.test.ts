import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  copyPassword,
  openUrl,
  entryDetail,
  entryHistory,
  listEntries,
  listTags,
  lockVault,
  onVaultSynced,
  revealPassword,
  refreshServer,
  serverStatus,
  syncVault,
  checkVault,
  type EntryDetail,
  type EntrySummary,
  type SyncReport,
  type SyncState,
} from "../lib/api";
import Toast from "../components/Toast.svelte";
import OpenVault from "./OpenVault.svelte";

vi.mock("../lib/api", () => ({
  listEntries: vi.fn(),
  listTags: vi.fn(),
  entryDetail: vi.fn(),
  entryHistory: vi.fn(),
  revealPassword: vi.fn(),
  copyPassword: vi.fn(),
  openUrl: vi.fn(),
  lockVault: vi.fn(),
  onVaultSynced: vi.fn(),
  refreshServer: vi.fn(),
  serverStatus: vi.fn(),
  syncVault: vi.fn(),
  checkVault: vi.fn(),
}));

const github: EntrySummary = {
  id: "1",
  kind: "login",
  title: "GitHub",
  username: "arthur",
  tags: ["dev", "perso"],
  hasPassword: true,
};
const wifi: EntrySummary = {
  id: "2",
  kind: "note",
  title: "Wi-Fi",
  username: null,
  tags: [],
  hasPassword: false,
};
const githubDetail: EntryDetail = {
  ...github,
  urls: ["https://github.com"],
  notes: "compte pro",
  createdAt: 0,
  updatedAt: Date.UTC(2026, 9, 23, 12, 0),
  versions: 2,
};
const wifiDetail: EntryDetail = {
  ...wifi,
  urls: [],
  notes: null,
  createdAt: 0,
  updatedAt: 0,
  versions: 1,
};

beforeEach(() => {
  vi.resetAllMocks();
  render(Toast);
  vi.mocked(listEntries).mockResolvedValue([github, wifi]);
  vi.mocked(listTags).mockResolvedValue([
    { tag: "dev", count: 1 },
    { tag: "perso", count: 1 },
  ]);
  vi.mocked(entryHistory).mockResolvedValue([]);
  vi.mocked(serverStatus).mockResolvedValue({ address: null, trouble: null, vaults: [] });
  vi.mocked(refreshServer).mockImplementation(() => serverStatus());
  vi.mocked(onVaultSynced).mockResolvedValue(vi.fn());
  vi.mocked(checkVault).mockResolvedValue();
  vi.mocked(entryDetail).mockImplementation((id) =>
    Promise.resolve(id === "1" ? githubDetail : wifiDetail),
  );
});

afterEach(() => {
  vi.useRealTimers();
});

function show(props: Partial<{ onLocked: () => void; onExpired: () => void }> = {}) {
  const onLocked = props.onLocked ?? vi.fn();
  const onExpired = props.onExpired ?? vi.fn();
  render(OpenVault, {
    vault: { name: "perso", entries: 2 },
    onLocked,
    onExpired,
    onDeleted: vi.fn(),
  });
  return { onLocked, onExpired };
}

async function open(title: string) {
  await fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${title}`) }));
  return screen.findByRole("heading", { level: 3, name: title });
}

describe("OpenVault", () => {
  it("lists the entries without any password", async () => {
    show();
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("perso");
    expect(await screen.findByText("2 entrées")).toBeInTheDocument();
    expect(screen.getByText("#dev #perso")).toBeInTheDocument();
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
    expect(listEntries).toHaveBeenCalledWith("", null);
  });

  it("searches as the user types and filters by tag", async () => {
    show();
    await screen.findByText("2 entrées");
    vi.mocked(listEntries).mockResolvedValue([github]);
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "git" } });
    expect(await screen.findByText("1 entrée")).toBeInTheDocument();
    expect(listEntries).toHaveBeenLastCalledWith("git", null);
    await fireEvent.change(screen.getByRole("combobox"), { target: { value: "dev" } });
    await vi.waitFor(() => {
      expect(listEntries).toHaveBeenLastCalledWith("git", "dev");
    });
    vi.mocked(listEntries).mockResolvedValue([]);
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "rien" } });
    expect(await screen.findByText("Aucune entrée.")).toBeInTheDocument();
  });

  it("hides the tag filter when there is no tag", async () => {
    vi.mocked(listTags).mockResolvedValue([]);
    show();
    await screen.findByText("2 entrées");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("focuses the search with Ctrl+F", async () => {
    show();
    await screen.findByText("2 entrées");
    const search = screen.getByRole("searchbox");
    await fireEvent.keyDown(window, { key: "g", ctrlKey: true });
    expect(search).not.toHaveFocus();
    await fireEvent.keyDown(window, { key: "f" });
    expect(search).not.toHaveFocus();
    await fireEvent.keyDown(window, { key: "F", ctrlKey: true });
    expect(search).toHaveFocus();
    search.blur();
    await fireEvent.keyDown(window, { key: "f", metaKey: true });
    expect(search).toHaveFocus();
  });

  it("shows the detail of an entry and goes back to the list", async () => {
    show();
    await open("GitHub");
    expect(screen.getByText("Mot de passe", { selector: ".kind" })).toBeInTheDocument();
    expect(screen.getByText("arthur", { selector: "dd" })).toBeInTheDocument();
    expect(screen.getByText("••••••••")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Ouvrir https://github.com dans le navigateur" }));
    expect(openUrl).toHaveBeenCalledWith("https://github.com");
    expect(screen.getByText("compte pro")).toBeInTheDocument();
    expect(screen.getByText("dev, perso")).toBeInTheDocument();
    expect(screen.getByText(/Modifiée le 23 octobre 2026 .* · 2 versions précédentes/)).toBeInTheDocument();
    expect(entryDetail).toHaveBeenCalledWith("1");
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(screen.getByText("Choisissez une entrée dans la liste.")).toBeInTheDocument();
  });

  it("shows a note without the fields it does not have", async () => {
    show();
    await open("Wi-Fi");
    expect(screen.getByText("Note sécurisée", { selector: ".kind" })).toBeInTheDocument();
    expect(screen.queryByText("••••••••")).not.toBeInTheDocument();
    expect(screen.getByText(/1 version précédente/)).toBeInTheDocument();
  });

  it("says when an entry has no previous version", async () => {
    vi.mocked(entryDetail).mockResolvedValue({ ...wifiDetail, versions: 0 });
    show();
    await open("Wi-Fi");
    expect(screen.getByText(/Aucune version précédente/)).toBeInTheDocument();
  });

  it("reveals the password on demand, then hides it again", async () => {
    vi.mocked(revealPassword).mockResolvedValue("hunter2");
    show();
    await open("GitHub");
    vi.useFakeTimers();
    await fireEvent.click(screen.getByRole("button", { name: "Afficher" }));
    await vi.waitFor(() => {
      expect(screen.getByText("hunter2")).toBeInTheDocument();
    });
    expect(revealPassword).toHaveBeenCalledWith("1");
    await fireEvent.click(screen.getByRole("button", { name: "Masquer" }));
    expect(screen.queryByText("hunter2")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Afficher" }));
    await vi.waitFor(() => {
      expect(screen.getByText("hunter2")).toBeInTheDocument();
    });
    vi.advanceTimersByTime(30_000);
    await vi.waitFor(() => {
      expect(screen.queryByText("hunter2")).not.toBeInTheDocument();
    });
  });

  it("copies the password through the backend", async () => {
    vi.mocked(copyPassword).mockResolvedValue(20);
    show();
    await open("GitHub");
    vi.useFakeTimers();
    await fireEvent.click(screen.getByRole("button", { name: "Copier" }));
    await vi.waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent("Copié, effacé dans 20 s.");
    });
    expect(copyPassword).toHaveBeenCalledWith("1");
    vi.advanceTimersByTime(2_000);
    await fireEvent.click(screen.getByRole("button", { name: "Copier" }));
    await vi.waitFor(() => {
      expect(copyPassword).toHaveBeenCalledTimes(2);
    });
    vi.advanceTimersByTime(2_000);
    await Promise.resolve();
    expect(screen.getByRole("status")).toBeInTheDocument();
    vi.advanceTimersByTime(1_000);
    await vi.waitFor(() => {
      expect(screen.queryByRole("status")).not.toBeInTheDocument();
    });
  });

  it("reports the errors of the detail", async () => {
    vi.mocked(revealPassword).mockRejectedValue({ code: "noPassword", detail: "GitHub" });
    vi.mocked(copyPassword).mockRejectedValue({ code: "clipboard", detail: "occupé" });
    show();
    await open("GitHub");
    await fireEvent.click(screen.getByRole("button", { name: "Afficher" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "L'entrée « GitHub » n'a pas de mot de passe.",
    );
    await fireEvent.click(screen.getByRole("button", { name: "Copier" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Copie impossible : occupé");
    vi.mocked(openUrl).mockRejectedValue({ code: "urlNotOpened", detail: "aucun navigateur" });
    await fireEvent.click(screen.getByRole("button", { name: /^Ouvrir https:\/\/github.com/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Ouverture du navigateur impossible : aucun navigateur");
  });

  it("reports an entry that disappeared", async () => {
    vi.mocked(entryDetail).mockRejectedValue({ code: "entryNotFound", detail: "1" });
    show();
    await fireEvent.click(await screen.findByRole("button", { name: /^GitHub/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Cette entrée n'existe plus.");
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
  });

  it("clears a search error once the search works again", async () => {
    vi.mocked(listEntries).mockRejectedValue({ code: "unexpected", detail: "panne" });
    show();
    expect(await screen.findByRole("alert")).toHaveTextContent("Erreur : panne");
    vi.mocked(listEntries).mockResolvedValue([wifi]);
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "w" } });
    await vi.waitFor(() => {
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  it("leaves when the vault locked itself in the meantime", async () => {
    vi.mocked(listTags).mockRejectedValue({ code: "vaultLocked", detail: null });
    const { onExpired } = show();
    await vi.waitFor(() => {
      expect(onExpired).toHaveBeenCalled();
    });
  });

  it("locks the vault on demand", async () => {
    vi.mocked(lockVault).mockResolvedValue();
    const { onLocked } = show();
    await fireEvent.click(screen.getByRole("button", { name: "Verrouiller" }));
    await vi.waitFor(() => {
      expect(onLocked).toHaveBeenCalled();
    });
  });
});

describe("pagination on a wide screen", () => {
  function wideScreen(height: number) {
    const query = { matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() };
    vi.stubGlobal("matchMedia", vi.fn(() => query));
    vi.stubGlobal("innerHeight", height);
    return query;
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const many: EntrySummary[] = Array.from({ length: 7 }, (_, index) => ({
    ...wifi,
    id: String(index + 10),
    title: `Note ${String(index + 1)}`,
  }));

  it("shows one page at a time and starts again from the first page on a new search", async () => {
    const query = wideScreen(400);
    vi.mocked(listEntries).mockResolvedValue(many);
    const { unmount } = render(OpenVault, {
      vault: { name: "perso", entries: 7 },
      onLocked: vi.fn(),
      onExpired: vi.fn(),
      onDeleted: vi.fn(),
    });
    expect(await screen.findByText("Page 1 / 3")).toBeInTheDocument();
    expect(screen.getByText("7 entrées")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Note 3/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Note 4/ })).not.toBeInTheDocument();
    const previous = screen.getByRole("button", { name: "Page précédente" });
    const next = screen.getByRole("button", { name: "Page suivante" });
    expect(previous).toBeDisabled();
    await fireEvent.click(next);
    await fireEvent.click(next);
    expect(screen.getByText("Page 3 / 3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Note 7/ })).toBeInTheDocument();
    expect(next).toBeDisabled();
    await fireEvent.click(previous);
    expect(screen.getByText("Page 2 / 3")).toBeInTheDocument();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "Note" } });
    expect(await screen.findByText("Page 1 / 3")).toBeInTheDocument();
    vi.mocked(listEntries).mockResolvedValue(many.slice(0, 2));
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "Note 1" } });
    await vi.waitFor(() => {
      expect(screen.queryByRole("navigation", { name: "Pages des entrées" })).not.toBeInTheDocument();
    });
    unmount();
    expect(query.removeEventListener).toHaveBeenCalled();
  });

  it("keeps a valid page when the list gets shorter", async () => {
    wideScreen(400);
    vi.mocked(listEntries).mockResolvedValue(many);
    render(OpenVault, {
      vault: { name: "perso", entries: 7 },
      onLocked: vi.fn(),
      onExpired: vi.fn(),
      onDeleted: vi.fn(),
    });
    const next = await screen.findByRole("button", { name: "Page suivante" });
    await fireEvent.click(next);
    await fireEvent.click(next);
    vi.mocked(listEntries).mockResolvedValue(many.slice(0, 4));
    await fireEvent.change(screen.getByRole("combobox", { name: "Filtrer par tag" }), { target: { value: "dev" } });
    expect(await screen.findByText("Page 1 / 2")).toBeInTheDocument();
  });
});

describe("OpenVault and the server", () => {
  const connected = { address: "pswd.local:8443", trouble: null, vaults: [] };

  function report(state: SyncState, changed = false, vault = "perso"): SyncReport {
    return {
      vault,
      state,
      detail: state === "offline" ? "serveur injoignable" : null,
      changed,
      passwordChanged: false,
    };
  }

  /** Le gestionnaire passé à `onVaultSynced`, et la fonction qui arrête l'écoute. */
  function listenSyncs() {
    const stop = vi.fn();
    let handler: (report: SyncReport) => void = () => undefined;
    vi.mocked(onVaultSynced).mockImplementation((listener) => {
      handler = listener;
      return Promise.resolve(stop);
    });
    return { stop, send: (sent: SyncReport) => handler(sent) };
  }

  it("checks the server every 5 seconds while the page is visible, silently", async () => {
    vi.useFakeTimers({ toFake: ["setInterval", "clearInterval"] });
    vi.mocked(checkVault).mockRejectedValueOnce({ code: "serverError", detail: "serveur injoignable" });
    const { unmount } = render(OpenVault, {
      vault: { name: "perso", entries: 2 },
      onLocked: vi.fn(),
      onExpired: vi.fn(),
      onDeleted: vi.fn(),
    });
    await screen.findByText("2 entrées");
    expect(checkVault).not.toHaveBeenCalled();
    vi.advanceTimersByTime(4_999);
    expect(checkVault).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(checkVault).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(5_000);
    expect(checkVault).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    vi.advanceTimersByTime(5_000);
    expect(checkVault).toHaveBeenCalledTimes(2);
    visibility.mockRestore();
    unmount();
    vi.advanceTimersByTime(5_000);
    expect(checkVault).toHaveBeenCalledTimes(2);
  });

  it("offers no sync to a device without a server", async () => {
    show();
    await screen.findByText("2 entrées");
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();
  });

  it("stays usable without a readable server state", async () => {
    vi.mocked(serverStatus).mockRejectedValue({ code: "configInvalid", detail: "server.toml" });
    show();
    expect(await screen.findByText("2 entrées")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Synchroniser" })).not.toBeInTheDocument();
  });

  it("synchronizes on demand and shows what came from the server", async () => {
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockResolvedValue(report("synced", true));
    show();
    await screen.findByText("2 entrées");
    vi.mocked(listEntries).mockResolvedValue([github, wifi, { ...wifi, id: "3", title: "Banque" }]);
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(await screen.findByText("Coffre à jour.")).toBeInTheDocument();
    expect(await screen.findByText("3 entrées")).toBeInTheDocument();
  });

  it("keeps a sync problem on screen until the next sync succeeds", async () => {
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockResolvedValue(report("offline"));
    show();
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    const notice = await screen.findByText(
      "Synchro impossible : serveur injoignable. Les modifications seront publiées plus tard.",
    );
    expect(screen.queryByText("Coffre à jour.")).not.toBeInTheDocument();
    vi.mocked(syncVault).mockResolvedValue(report("synced"));
    await fireEvent.click(screen.getByRole("button", { name: "Synchroniser" }));
    await vi.waitFor(() => {
      expect(notice).not.toBeInTheDocument();
    });
  });

  it("returns to the list once the account is no longer a member", async () => {
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockResolvedValue(report("left"));
    const { onLocked } = show();
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(
      await screen.findByText("Vous n'êtes plus membre : coffre retiré de cet appareil."),
    ).toBeInTheDocument();
    expect(onLocked).toHaveBeenCalled();
  });

  it("returns to the list once the vault was deleted everywhere from another device", async () => {
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockResolvedValue(report("deleted"));
    const { onLocked } = show();
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    expect(
      await screen.findByText(
        "Coffre supprimé depuis un autre appareil.",
      ),
    ).toBeInTheDocument();
    expect(onLocked).toHaveBeenCalled();
  });

  it("returns to the list when the vault locked itself during the sync", async () => {
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockRejectedValue({ code: "vaultLocked", detail: null });
    const { onExpired } = show();
    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    await vi.waitFor(() => {
      expect(onExpired).toHaveBeenCalled();
    });
  });

  it("says when the master password was changed on another device", async () => {
    const syncs = listenSyncs();
    vi.mocked(serverStatus).mockResolvedValue(connected);
    vi.mocked(syncVault).mockResolvedValue({ ...report("synced"), passwordChanged: true });
    show();
    await screen.findByText("2 entrées");
    syncs.send({ ...report("synced"), passwordChanged: true });
    const changed = "Mot de passe maître changé sur un autre appareil.";
    expect(await screen.findByText(changed)).toBeInTheDocument();

    await fireEvent.click(await screen.findByRole("button", { name: "Synchroniser" }));
    await vi.waitFor(() => {
      expect(screen.getByRole("button", { name: "Synchroniser" })).toBeEnabled();
    });
    expect(syncVault).toHaveBeenCalled();
    expect(screen.getByText(changed)).toBeInTheDocument();
    expect(screen.queryByText("Coffre à jour.")).not.toBeInTheDocument();
  });

  it("follows the automatic syncs of this vault only", async () => {
    const syncs = listenSyncs();
    const { unmount } = render(OpenVault, {
      vault: { name: "perso", entries: 2 },
      onLocked: vi.fn(),
      onExpired: vi.fn(),
      onDeleted: vi.fn(),
    });
    await screen.findByText("2 entrées");
    syncs.send(report("offline", false, "travail"));
    syncs.send(report("conflict"));
    expect(
      await screen.findByText("Le coffre a changé sur le serveur à chaque essai : synchronisez de nouveau."),
    ).toBeInTheDocument();
    vi.mocked(listEntries).mockResolvedValue([github]);
    syncs.send(report("synced", true));
    expect(await screen.findByText("1 entrée")).toBeInTheDocument();
    expect(screen.queryByText(/Le coffre a changé/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Synchro impossible/)).not.toBeInTheDocument();
    unmount();
    await vi.waitFor(() => {
      expect(syncs.stop).toHaveBeenCalled();
    });
  });
});
