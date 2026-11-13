<script lang="ts">
  import { onMount, tick } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { openUrl } from "../lib/api";
  import { onBack } from "../lib/back";
  import { errorMessage } from "../lib/errors";
  import { helpPage, linkTarget, type HelpPage } from "../lib/help";
  import { texts } from "../lib/i18n";

  interface Props {
    onClose: () => void;
  }

  const { onClose }: Props = $props();

  let page = $state<HelpPage>("readme");
  let problem = $state<string | null>(null);
  let article: HTMLElement | undefined = $state();

  async function show(next: HelpPage, anchor: string | null) {
    page = next;
    problem = null;
    await tick();
    if (anchor === null) {
      article?.scrollIntoView({ block: "start" });
    } else {
      scrollTo(anchor);
    }
  }

  function scrollTo(id: string) {
    document.getElementById(id)?.scrollIntoView({ block: "start" });
  }

  // Le guide revient au README ; depuis le README, l'écran se ferme.
  function backToReadme(): boolean {
    if (page === "guide") {
      void show("readme", null);
      return true;
    }
    return false;
  }

  function back() {
    if (!backToReadme()) {
      onClose();
    }
  }

  async function follow(event: MouseEvent) {
    const link = (event.target as Element).closest("a");
    const href = link?.getAttribute("href");
    if (href === null || href === undefined) {
      return;
    }
    // Rien ne s'ouvre dans la vue de l'application : un lien web part dans le navigateur.
    event.preventDefault();
    const target = linkTarget(href);
    if (target.kind === "page") {
      await show(target.page, target.anchor);
    } else if (target.kind === "anchor") {
      scrollTo(target.id);
    } else if (target.kind === "web") {
      try {
        await openUrl(target.url);
        problem = null;
      } catch (error) {
        problem = errorMessage(error);
      }
    }
  }

  onMount(() => onBack(backToReadme));
</script>

<section class="panel help" aria-label={texts.help}>
  <button type="button" class="back icon-button" aria-label={texts.back} title={texts.back} onclick={back}>
    <Icon name="back" />
  </button>
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <!-- Le clic d'un lien remonte jusqu'ici, au clavier (Entrée) comme à la souris : `follow` le suit. -->
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <article bind:this={article} class="doc" onclick={follow}>
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- documentation du dépôt, embarquée à la construction : aucun contenu venu de l'extérieur. -->
    {@html helpPage(page)}
  </article>
</section>

<style>
  /* Toute la largeur de l'application : les tableaux du guide y tiennent sans défiler. */
  .help {
    max-width: none;
  }

  /* Le guide est long : « Retour » reste à portée en haut de l'écran pendant la lecture. */
  .back {
    position: sticky;
    top: max(var(--space-s), env(safe-area-inset-top));
    z-index: 1;
    margin-bottom: 1.5rem;
    background: var(--surface-solid);
    box-shadow: var(--shadow);
  }

  .doc {
    line-height: 1.6;
    overflow-wrap: anywhere;
  }

  .doc :global(h1) {
    margin-top: 0;
    font-size: 1.5rem;
  }

  .doc :global(h2) {
    margin-top: var(--space-l);
    padding-bottom: var(--space-s);
    border-bottom: 1px solid var(--border);
    font-size: 1.25rem;
  }

  .doc :global(h3) {
    font-size: 1.05rem;
  }

  /* Le défilement vers une section laisse la place du bouton « Retour ». */
  .doc :global(:is(h1, h2, h3)) {
    scroll-margin-top: 4rem;
  }

  .doc :global(a) {
    color: var(--accent);
  }

  .doc :global(blockquote) {
    margin: var(--space-m) 0;
    padding: var(--space-s) var(--space-m);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent-soft);
  }

  .doc :global(blockquote p) {
    margin: 0;
  }

  .doc :global(code) {
    padding: 0.1em 0.35em;
    border-radius: 0.375rem;
    background: var(--surface-hover);
    font-size: 0.9em;
  }

  .doc :global(pre) {
    padding: var(--space-m);
    overflow-x: auto;
    border-radius: var(--radius);
    background: var(--surface-hover);
    font-size: 0.875rem;
  }

  .doc :global(pre code) {
    padding: 0;
    background: none;
    overflow-wrap: normal;
  }

  /* Le tableau occupe toute la carte ; trop large pour l'écran, il défile dans son cadre. */
  .doc :global(.table) {
    margin: var(--space-m) 0;
    overflow-x: auto;
  }

  .doc :global(table) {
    width: 100%;
    border-collapse: collapse;
  }

  .doc :global(th),
  .doc :global(td) {
    padding: var(--space-s);
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
    overflow-wrap: normal;
  }

  .doc :global(th) {
    background: var(--surface-hover);
  }

  /* Écran étroit (téléphone) : texte et tableaux plus serrés, colonnes jamais écrasées. */
  @media (max-width: 48rem) {
    .doc {
      line-height: 1.5;
    }

    .doc :global(h1) {
      font-size: 1.3rem;
    }

    .doc :global(h2) {
      margin-top: var(--space-m);
      font-size: 1.125rem;
    }

    .doc :global(h3) {
      font-size: 1rem;
    }

    .doc :global(table) {
      font-size: 0.875rem;
    }

    .doc :global(th),
    .doc :global(td) {
      min-width: 7rem;
      padding: 0.375rem;
    }

    .doc :global(ul),
    .doc :global(ol) {
      padding-left: 1.25rem;
    }
  }
</style>
