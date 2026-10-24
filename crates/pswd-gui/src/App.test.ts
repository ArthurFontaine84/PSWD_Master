import { render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import { listVaults } from "./lib/api";

vi.mock("./lib/api", () => ({ listVaults: vi.fn() }));

describe("App", () => {
  beforeEach(() => {
    vi.mocked(listVaults).mockReset();
  });

  it("lists the vaults of this device", async () => {
    vi.mocked(listVaults).mockResolvedValue(["perso", "travail"]);
    render(App);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("PSWD Master");
    expect(screen.getByText("Chargement…")).toBeInTheDocument();
    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual(["perso", "travail"]);
  });

  it("says when there is no vault", async () => {
    vi.mocked(listVaults).mockResolvedValue([]);
    render(App);
    expect(await screen.findByText("Aucun coffre sur cet appareil.")).toBeInTheDocument();
  });

  it("shows a translated error", async () => {
    vi.mocked(listVaults).mockRejectedValue({ code: "configInvalid", detail: "ligne 2" });
    render(App);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Configuration invalide : ligne 2",
    );
  });
});
