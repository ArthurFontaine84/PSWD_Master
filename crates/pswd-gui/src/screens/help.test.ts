import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openUrl } from "../lib/api";
import { goBack } from "../lib/back";
import { helpPage, toHtml, type HelpPage } from "../lib/help";
import Help from "./Help.svelte";

vi.mock("../lib/api", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/api/app", () => ({ exit: vi.fn(), onBackButtonPress: vi.fn() }));
vi.mock("../lib/help", async (original) => ({
  ...(await original<typeof import("../lib/help")>()),
  helpPage: vi.fn(),
}));

const pages: Record<HelpPage, string> = {
  readme: [
    "# PSWD Master",
    "[Cliquez ici pour accéder au guide](docs/GUIDE.md)",
    "[Bon à savoir](docs/GUIDE.md#bon-à-savoir)",
    "[Site](https://www.duckdns.org)",
    "[Conception](docs/SPEC.md)",
    "Un paragraphe.",
  ].join("\n\n"),
  guide: ["# Guide", "[Plus bas](#bon-à-savoir)", "## Bon à savoir", "Fin."].join("\n\n"),
};

const scrolled = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(helpPage).mockImplementation((page) => toHtml(pages[page]));
  Element.prototype.scrollIntoView = function (this: Element) {
    scrolled(this);
  };
});

afterEach(() => {
  Reflect.deleteProperty(Element.prototype, "scrollIntoView");
});

function link(name: string): HTMLElement {
  return screen.getByRole("link", { name });
}

describe("Help", () => {
  it("shows the README first, then the guide through its link", async () => {
    render(Help, { onClose: vi.fn() });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("PSWD Master");
    await fireEvent.click(link("Cliquez ici pour accéder au guide"));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Guide");
    expect(scrolled).toHaveBeenCalledWith(screen.getByRole("article"));
  });

  it("opens the guide at the section named by the link", async () => {
    render(Help, { onClose: vi.fn() });
    await fireEvent.click(link("Bon à savoir"));
    expect(scrolled).toHaveBeenCalledWith(screen.getByRole("heading", { name: "Bon à savoir" }));
  });

  it("scrolls to a section of the same page", async () => {
    render(Help, { onClose: vi.fn() });
    await fireEvent.click(link("Cliquez ici pour accéder au guide"));
    scrolled.mockClear();
    await fireEvent.click(link("Plus bas"));
    expect(scrolled).toHaveBeenCalledWith(screen.getByRole("heading", { name: "Bon à savoir" }));
  });

  it("goes back from the guide to the README, then closes", async () => {
    const onClose = vi.fn();
    render(Help, { onClose });
    await fireEvent.click(link("Cliquez ici pour accéder au guide"));
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("PSWD Master");
    expect(onClose).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Retour" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("answers the back button of Android from the guide only", async () => {
    const onClose = vi.fn();
    render(Help, { onClose });
    await fireEvent.click(link("Cliquez ici pour accéder au guide"));
    goBack();
    expect(await screen.findByRole("heading", { level: 1, name: "PSWD Master" })).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("opens web addresses in the browser, never in the application", async () => {
    vi.mocked(openUrl).mockResolvedValue();
    render(Help, { onClose: vi.fn() });
    const site = link("Site");
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    site.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(openUrl).toHaveBeenCalledWith("https://www.duckdns.org");
  });

  it("says why the browser did not open, until the next try works", async () => {
    vi.mocked(openUrl).mockRejectedValueOnce({ code: "urlNotOpened", detail: "aucun navigateur" });
    render(Help, { onClose: vi.fn() });
    await fireEvent.click(link("Site"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ouverture du navigateur impossible : aucun navigateur",
    );
    vi.mocked(openUrl).mockResolvedValue();
    await fireEvent.click(link("Site"));
    await vi.waitFor(() => {
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  it("follows neither a document left out of the application, nor a click outside a link", async () => {
    render(Help, { onClose: vi.fn() });
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    link("Conception").dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    await fireEvent.click(screen.getByText("Un paragraphe."));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("PSWD Master");
    expect(openUrl).not.toHaveBeenCalled();
    expect(scrolled).not.toHaveBeenCalled();
  });
});
