<script lang="ts" module>
  import type { IconName } from "./Icon.svelte";

  export interface MenuItem {
    label: string;
    icon: IconName;
    onSelect: () => void;
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import { onBack } from "../lib/back";
  import { texts } from "../lib/i18n";
  import { motion } from "../lib/motion";
  import Icon from "./Icon.svelte";

  interface Props {
    items: MenuItem[];
  }

  const { items }: Props = $props();

  let open = $state(false);
  let anchor: HTMLElement | undefined = $state();

  function choose(item: MenuItem) {
    open = false;
    item.onSelect();
  }

  /** Un clic hors du menu le referme. */
  function closeOutside(event: MouseEvent) {
    if (open && !anchor?.contains(event.target as Node)) {
      open = false;
    }
  }

  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === "Escape") {
      open = false;
    }
  }

  // Le bouton retour d'Android referme d'abord le menu.
  onMount(() =>
    onBack(() => {
      if (!open) {
        return false;
      }
      open = false;
      return true;
    }),
  );
</script>

<svelte:window onclick={closeOutside} onkeydown={closeOnEscape} />

<div class="anchor" bind:this={anchor}>
  <button
    type="button"
    class="icon-button burger"
    class:open
    aria-label={texts.menu}
    title={texts.menu}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <span class="bars" aria-hidden="true"><span></span><span></span><span></span></span>
  </button>
  {#if open}
    <div class="menu" role="menu" aria-label={texts.menu} transition:scale={{ start: 0.9, duration: motion(180) }}>
      {#each items as item (item.label)}
        <button type="button" role="menuitem" onclick={() => choose(item)}>
          <Icon name={item.icon} />
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .anchor {
    position: relative;
  }

  /* Trois barres qui se croisent en « × » à l'ouverture. */
  .bars {
    position: relative;
    display: block;
    width: 1.25rem;
    height: 0.875rem;
  }

  .bars span {
    position: absolute;
    left: 0;
    width: 100%;
    height: 2px;
    border-radius: 1px;
    background: currentColor;
    transition:
      transform 0.3s var(--ease),
      opacity 0.2s var(--ease),
      top 0.3s var(--ease);
  }

  .bars span:nth-child(1) {
    top: 0;
  }

  .bars span:nth-child(2) {
    top: calc(50% - 1px);
  }

  .bars span:nth-child(3) {
    top: calc(100% - 2px);
  }

  .open .bars span:nth-child(1),
  .open .bars span:nth-child(3) {
    top: calc(50% - 1px);
  }

  .open .bars span:nth-child(1) {
    transform: rotate(45deg);
  }

  .open .bars span:nth-child(2) {
    opacity: 0;
    transform: scaleX(0);
  }

  .open .bars span:nth-child(3) {
    transform: rotate(-45deg);
  }

  /* Le menu se déploie depuis le bouton, à droite de l'écran. */
  .menu {
    position: absolute;
    top: calc(100% + var(--space-s));
    right: 0;
    z-index: 10;
    display: grid;
    min-width: 12rem;
    padding: 0.375rem;
    transform-origin: top right;
    background: var(--surface-solid);
    border: 1px solid var(--border);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow-raised);
  }

  .menu button {
    justify-content: flex-start;
    border: none;
    box-shadow: none;
    font-weight: 500;
    white-space: nowrap;
  }

  .menu :global(.icon) {
    color: var(--accent);
  }

  @media (prefers-reduced-motion: reduce) {
    .bars span {
      transition: none;
    }
  }
</style>
