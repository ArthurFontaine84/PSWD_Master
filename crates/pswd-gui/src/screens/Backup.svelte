<script lang="ts">
  import { backupFileName, backupToFile, listVaults } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupDestination } from "../lib/files";
  import { texts } from "../lib/i18n";
  import VaultChoice from "../components/VaultChoice.svelte";

  interface Props {
    /** Sauvegarde réussie : la confirmation à afficher. */
    onBackedUp: (message: string) => void;
    onCancel: () => void;
  }

  const { onBackedUp, onCancel }: Props = $props();
  const vaults = listVaults();

  /** Les coffres cochés, à sauvegarder. */
  let selection = $state<string[]>([]);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function backup(names: string[]) {
    problem = null;
    busy = true;
    try {
      const path = await pickBackupDestination(texts.backupFileTitle, await backupFileName());
      if (path !== null) {
        const chosen = names.filter((name) => selection.includes(name));
        onBackedUp(texts.backedUp(await backupToFile(path, chosen)));
      }
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel" aria-labelledby="backup-title">
  <h2 id="backup-title">{texts.backupTitle}</h2>
  <p class="explanation">{texts.backupExplanation}</p>
  {#await vaults}
    <p class="explanation">{texts.loading}</p>
  {:then names}
    <VaultChoice {names} label={texts.vaultsToBackUp} bind:selection />
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" onclick={onCancel}>{texts.cancel}</button>
      <button
        type="button"
        class="primary"
        disabled={selection.length === 0 || busy}
        onclick={() => backup(names)}
      >
        {busy ? texts.backingUp : texts.backupSelection(selection.length)}
      </button>
    </div>
  {:catch error}
    <p role="alert">{errorMessage(error)}</p>
    <div class="actions">
      <button type="button" onclick={onCancel}>{texts.cancel}</button>
    </div>
  {/await}
</section>

<style>
  .explanation {
    color: var(--text-muted);
  }
</style>
