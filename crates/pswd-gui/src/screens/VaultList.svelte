<script lang="ts">
  import { backupFileName, backupToFile, listVaults } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupDestination } from "../lib/files";
  import { texts } from "../lib/i18n";

  interface Props {
    notice?: string;
    onOpen: (name: string) => void;
    onCreate: () => void;
    onRestore: () => void;
    onSettings: () => void;
  }

  const { notice, onOpen, onCreate, onRestore, onSettings }: Props = $props();
  const vaults = listVaults();

  let backupResult = $state<string | null>(null);
  let backupProblem = $state<string | null>(null);

  async function backup() {
    backupResult = null;
    backupProblem = null;
    try {
      const path = await pickBackupDestination(texts.backupFileTitle, await backupFileName());
      if (path === null) {
        return;
      }
      backupResult = texts.backedUp(await backupToFile(path), path);
    } catch (error) {
      backupProblem = errorMessage(error);
    }
  }
</script>

<section aria-labelledby="vaults-title">
  <header>
    <h2 id="vaults-title">{texts.vaults}</h2>
    <button type="button" onclick={onSettings}>{texts.settings}</button>
  </header>
  {#if notice}
    <p role="status" class="notice">{notice}</p>
  {/if}
  {#await vaults}
    <p>{texts.loading}</p>
  {:then names}
    {#if names.length === 0}
      <p>{texts.noVault}</p>
    {:else}
      <ul>
        {#each names as name (name)}
          <li><button type="button" class="vault" onclick={() => onOpen(name)}>{name}</button></li>
        {/each}
      </ul>
    {/if}
    {#if backupResult}
      <p role="status">{backupResult}</p>
    {/if}
    {#if backupProblem}
      <p role="alert">{backupProblem}</p>
    {/if}
    <div class="actions">
      {#if names.length > 0}
        <button type="button" onclick={backup}>{texts.backup}</button>
      {/if}
      <button type="button" onclick={onRestore}>{texts.restore}</button>
      <button type="button" class="primary" onclick={onCreate}>{texts.newVault}</button>
    </div>
  {:catch error}
    <p role="alert">{errorMessage(error)}</p>
  {/await}
</section>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-m);
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--space-m);
  }

  .vault {
    width: 100%;
    text-align: left;
    padding: var(--space-m);
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: none;
  }

  .vault:focus-visible {
    background: var(--surface);
  }

  /* Sur un écran tactile, le survol resterait affiché après le toucher. */
  @media (hover: hover) {
    .vault:hover {
      background: var(--surface);
    }
  }
</style>
