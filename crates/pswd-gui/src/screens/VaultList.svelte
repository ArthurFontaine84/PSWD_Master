<script lang="ts">
  import { onDestroy } from "svelte";
  import { fly, slide } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import { backupFileName, backupToFile, listVaults } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupDestination } from "../lib/files";
  import { texts } from "../lib/i18n";
  import { motion } from "../lib/motion";

  interface Props {
    notice?: string;
    onOpen: (name: string) => void;
    onRestore: () => void;
  }

  const { notice, onOpen, onRestore }: Props = $props();
  const vaults = listVaults();
  /** Sans coffre lisible, rien à sauvegarder : l'erreur s'affiche sous la liste. */
  const canBackUp = vaults.then(
    (names) => names.length > 0,
    () => false,
  );

  /** Durée d'affichage du message de sauvegarde réussie (ms). */
  const RESULT_DURATION = 5_000;

  let backupResult = $state<string | null>(null);
  let backupProblem = $state<string | null>(null);
  let resultTimer: ReturnType<typeof setTimeout> | undefined;

  async function backup() {
    backupResult = null;
    backupProblem = null;
    try {
      const path = await pickBackupDestination(texts.backupFileTitle, await backupFileName());
      if (path === null) {
        return;
      }
      backupResult = texts.backedUp(await backupToFile(path));
      clearTimeout(resultTimer);
      resultTimer = setTimeout(() => (backupResult = null), RESULT_DURATION);
    } catch (error) {
      backupProblem = errorMessage(error);
    }
  }

  onDestroy(() => {
    clearTimeout(resultTimer);
  });
</script>

<section aria-labelledby="vaults-title">
  <div class="toolbar">
    {#await canBackUp then shown}
      {#if shown}
        <button type="button" onclick={backup}><Icon name="download" />{texts.backup}</button>
      {/if}
    {/await}
    <button type="button" onclick={onRestore}><Icon name="restore" />{texts.restore}</button>
  </div>
  {#if notice}
    <p role="status" class="notice">{notice}</p>
  {/if}
  {#if backupResult}
    <p role="status" class="notice" transition:slide={{ duration: motion(250) }}>{backupResult}</p>
  {/if}
  {#if backupProblem}
    <p role="alert">{backupProblem}</p>
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
              <span class="name">{name}</span>
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
    display: inline-flex;
    padding: 0.5rem;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .name {
    flex: 1;
    text-align: left;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  /* Écran large : les coffres sur plusieurs colonnes. */
  @media (min-width: 48rem) {
    ul {
      grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    }
  }
</style>
