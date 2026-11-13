// Documentation de l'utilisateur, embarquée à la construction : le README et le guide du dépôt.
import { Marked, type Tokens } from "marked";

// EXTENSION(anglais): un README et un guide par langue.
export type HelpPage = "readme" | "guide";

/** Où mène un lien de la documentation. */
export type LinkTarget =
  | { kind: "page"; page: HelpPage; anchor: string | null }
  | { kind: "anchor"; id: string }
  | { kind: "web"; url: string }
  | { kind: "none" };

/** Identifiant d'un titre, calculé comme GitHub : les liens du sommaire marchent aux deux endroits. */
export function slug(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, "")
    .replace(/ /g, "-");
}

const marked = new Marked({
  renderer: {
    heading({ tokens, depth, text }: Tokens.Heading) {
      return `<h${depth} id="${slug(text)}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
    },
  },
  // Un tableau trop large pour l'écran défile dans son cadre, sans élargir la page.
  hooks: {
    postprocess: (html: string) =>
      html.replace(/<table>/g, '<div class="table"><table>').replace(/<\/table>/g, "</table></div>"),
  },
});

/** Page HTML d'un document Markdown. */
export function toHtml(markdown: string): string {
  return marked.parse(markdown, { async: false });
}

const sources: Record<HelpPage, string> = { readme: __HELP_README__, guide: __HELP_GUIDE__ };

/** Page HTML de la documentation. */
export function helpPage(page: HelpPage): string {
  return toHtml(sources[page]);
}

/** Destination d'un lien de la documentation. */
export function linkTarget(href: string): LinkTarget {
  if (href.startsWith("#")) {
    return { kind: "anchor", id: decodeURIComponent(href.slice(1)) };
  }
  if (/^https?:\/\//i.test(href)) {
    return { kind: "web", url: href };
  }
  const [path = "", anchor] = href.split("#");
  const file = path.split("/").pop();
  const page: HelpPage | null = file === "README.md" ? "readme" : file === "GUIDE.md" ? "guide" : null;
  if (page === null) {
    return { kind: "none" };
  }
  return { kind: "page", page, anchor: anchor === undefined ? null : decodeURIComponent(anchor) };
}
