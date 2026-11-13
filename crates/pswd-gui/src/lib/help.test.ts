import { describe, expect, it } from "vitest";
import { helpPage, linkTarget, slug, toHtml, type HelpPage } from "./help";

/** Les liens d'une page, tels qu'écrits dans le document. */
function links(page: HelpPage): string[] {
  const doc = new DOMParser().parseFromString(helpPage(page), "text/html");
  return [...doc.querySelectorAll("a")].map((link) => link.getAttribute("href") ?? "");
}

function ids(page: HelpPage): string[] {
  const doc = new DOMParser().parseFromString(helpPage(page), "text/html");
  return [...doc.querySelectorAll("[id]")].map((element) => element.id);
}

describe("slug", () => {
  it("computes the anchors of GitHub", () => {
    expect(slug("2. Quel usage choisir ?")).toBe("2-quel-usage-choisir-");
    expect(slug("8. Installer l'application sur un autre appareil")).toBe(
      "8-installer-lapplication-sur-un-autre-appareil",
    );
    expect(slug("3. Utiliser PSWD Master sur un appareil (local)")).toBe(
      "3-utiliser-pswd-master-sur-un-appareil-local",
    );
    expect(slug("6. Bon à savoir")).toBe("6-bon-à-savoir");
    expect(slug("Le `port` **réseau**")).toBe("le-port-réseau");
  });
});

describe("toHtml", () => {
  it("gives every heading its anchor, and keeps its formatting", () => {
    expect(toHtml("## Titre **gras**")).toBe('<h2 id="titre-gras">Titre <strong>gras</strong></h2>\n');
  });

  it("puts every table in a frame of its own, which scrolls on a narrow screen", () => {
    const html = toHtml("| A | B |\n|---|---|\n| 1 | 2 |\n\nTexte\n\n| C |\n|---|\n| 3 |");
    expect(html.match(/<div class="table"><table>/g)).toHaveLength(2);
    expect(html.match(/<\/table><\/div>/g)).toHaveLength(2);
  });

  it("does not turn an address written as code into a link", () => {
    expect(toHtml("`https://github.com`")).not.toContain("<a");
    expect(toHtml("https://github.com")).toContain('<a href="https://github.com"');
  });
});

describe("linkTarget", () => {
  it("scrolls to an anchor of the page", () => {
    expect(linkTarget("#6-bon-%C3%A0-savoir")).toEqual({ kind: "anchor", id: "6-bon-à-savoir" });
  });

  it("opens web addresses in the browser", () => {
    expect(linkTarget("https://www.duckdns.org")).toEqual({ kind: "web", url: "https://www.duckdns.org" });
    expect(linkTarget("HTTP://exemple.fr")).toEqual({ kind: "web", url: "HTTP://exemple.fr" });
  });

  it("switches between the README and the guide, at an anchor if given", () => {
    expect(linkTarget("docs/GUIDE.md")).toEqual({ kind: "page", page: "guide", anchor: null });
    expect(linkTarget("../README.md")).toEqual({ kind: "page", page: "readme", anchor: null });
    expect(linkTarget("GUIDE.md#5-installer-le-serveur")).toEqual({
      kind: "page",
      page: "guide",
      anchor: "5-installer-le-serveur",
    });
  });

  it("ignores the documents that are not in the application", () => {
    expect(linkTarget("docs/SPEC.md")).toEqual({ kind: "none" });
    expect(linkTarget("mailto:moi@exemple.fr")).toEqual({ kind: "none" });
  });
});

describe("helpPage", () => {
  it("leads from the README to the guide", () => {
    expect(links("readme").map(linkTarget)).toContainEqual({ kind: "page", page: "guide", anchor: null });
  });

  it("only has links that work in the application", () => {
    const pages: HelpPage[] = ["readme", "guide"];
    for (const page of pages) {
      for (const href of links(page)) {
        const target = linkTarget(href);
        expect(target.kind, href).not.toBe("none");
        if (target.kind === "anchor") {
          expect(ids(page), href).toContain(target.id);
        }
        if (target.kind === "page" && target.anchor !== null) {
          expect(ids(target.page), href).toContain(target.anchor);
        }
      }
    }
  });
});
