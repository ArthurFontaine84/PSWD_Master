<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { texts } from "../lib/i18n";
  import { pageCount, pageOf, pageSizeFor, remSize, watchWide, type ListLayout } from "../lib/layout";
  import { motion } from "../lib/motion";
  import Icon from "./Icon.svelte";

  interface Props {
    items: T[];
    layout: ListLayout;
    /** Nom de la navigation entre les pages. */
    label: string;
    /** Page affichée, numérotée à partir de 0 ; remise à 0 par le parent après une recherche. */
    page?: number;
    /** Affiche les éléments de la page. */
    children: Snippet<[T[]]>;
  }

  let { items, layout, label, page = $bindable(0), children }: Props = $props();

  /** Écran large : la liste est découpée en pages qui tiennent dans la fenêtre ; sinon, elle défile. */
  let wide = $state(false);
  let windowHeight = $state(0);
  const size = $derived(pageSizeFor(windowHeight / remSize(), layout));
  const pages = $derived(wide ? pageCount(items.length, size) : 1);
  /** Page ramenée dans les bornes quand la liste raccourcit. */
  const shown = $derived(Math.min(page, pages - 1));
  const visible = $derived(wide ? pageOf(items, shown, size) : items);

  $effect(() => watchWide((value) => (wide = value)));
</script>

<svelte:window bind:innerHeight={windowHeight} />

{#key shown}
  <div in:fly={{ x: 12, duration: motion(200) }}>
    {@render children(visible)}
  </div>
{/key}
{#if pages > 1}
  <nav class="pager" aria-label={label}>
    <button
      type="button"
      class="icon-button"
      aria-label={texts.previousPage}
      title={texts.previousPage}
      disabled={shown === 0}
      onclick={() => (page = shown - 1)}
    >
      <Icon name="previous" />
    </button>
    <span>{texts.pageOf(shown + 1, pages)}</span>
    <button
      type="button"
      class="icon-button"
      aria-label={texts.nextPage}
      title={texts.nextPage}
      disabled={shown === pages - 1}
      onclick={() => (page = shown + 1)}
    >
      <Icon name="chevron" />
    </button>
  </nav>
{/if}

<style>
  .pager {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-m);
    margin-top: var(--space-m);
    color: var(--text-muted);
    font-size: 0.875rem;
    font-weight: 600;
  }
</style>
