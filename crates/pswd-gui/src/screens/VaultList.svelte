<script lang="ts">
  import { fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import { listVaults } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { motion } from "../lib/motion";

  interface Props {
    notice?: string;
    onOpen: (name: string) => void;
    onBackup: () => void;
    onRestore: () => void;
  }

  const { notice, onOpen, onBackup, onRestore }: Props = $props();
  const vaults = listVaults();
  /** Sans coffre lisible, rien à sauvegarder : l'erreur s'affiche sous la liste. */
  const canBackUp = vaults.then(
    (names) => names.length > 0,
    () => false,
  );
</script>

<section aria-labelledby="vaults-title">
  <div class="toolbar">
    {#await canBackUp then shown}
      {#if shown}
        <button type="button" onclick={onBackup}><Icon name="download" />{texts.backup}</button>
      {/if}
    {/await}
    <button type="button" onclick={onRestore}><Icon name="restore" />{texts.restore}</button>
  </div>
  {#if notice}
    <p role="status" class="notice">{notice}</p>
  {/if}
  <h2 id="vaults-title">{texts.vaults}</h2>
  {#await vaults}
    <p class="muted">{texts.loading}</p>
  {:then names}
    {#if names.length === 0}
      <p class="muted">{texts.noVault}</p>
    {:else}
      <ul>
        {#each names as name, index (name)}
          <li in:fly={{ y: 12, duration: motion(300), delay: motion(Math.min(index, 8) * 40) }}>
            <button type="button" class="vault" onclick={() => onOpen(name)}>
              <span class="badge"><Icon name="lock" /></span>
              <span class="name" title={name}>{name}</span>
              <Icon name="chevron" />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:catch error}
    <p role="alert">{errorMessage(error)}</p>
  {/await}
</section>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-s);
  }

  h2 {
    margin: var(--space-l) 0 var(--space-m);
  }

  .muted {
    color: var(--text-muted);
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: var(--space-s);
  }

  /* Sans cela, un nom long élargirait sa colonne au lieu d'être coupé. */
  li {
    min-width: 0;
  }

  .vault {
    width: 100%;
    justify-content: flex-start;
    gap: var(--space-m);
    padding: var(--space-m);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    color: var(--text-muted);
    -webkit-backdrop-filter: var(--blur);
    backdrop-filter: var(--blur);
  }

  @media (hover: hover) {
    .vault:hover {
      transform: translateY(-2px);
      box-shadow: var(--shadow-raised);
    }
  }

  .badge {
    flex: none;
    display: inline-flex;
    padding: 0.5rem;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    color: var(--text);
    /* Une seule ligne, coupée par « … » : toutes les cartes gardent la même taille. */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Écran large : les coffres sur plusieurs colonnes. */
  @media (min-width: 48rem) {
    ul {
      grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    }
  }

  /* Très grand écran : des cartes plus hautes et plus espacées. */
  @media (min-width: 100rem) and (min-height: 56rem) {
    ul {
      gap: var(--space-m);
    }

    .vault {
      padding: var(--space-l) var(--space-m);
      font-size: 1.125rem;
    }
  }
</style>
