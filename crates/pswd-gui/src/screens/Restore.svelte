<script lang="ts">
  import { backupFileContents, restoreFromFile } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { displayedFileName } from "../lib/fileName";
  import { pickBackupFile } from "../lib/files";
  import { texts } from "../lib/i18n";
  import VaultChoice from "../components/VaultChoice.svelte";

  interface Props {
    /** Restauration réussie : la confirmation à afficher. */
    onRestored: (message: string) => void;
    onCancel: () => void;
  }

  const { onRestored, onCancel }: Props = $props();

  /** Le fichier `.pswd` choisi. */
  let source = $state<string | null>(null);
  let names = $state<string[]>([]);
  /** Les coffres cochés, à restaurer. */
  let selection = $state<string[]>([]);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  const sourceName = $derived(source === null ? null : displayedFileName(source));

  async function choose() {
    problem = null;
    try {
      const path = await pickBackupFile(texts.restoreFileTitle);
      if (path === null) {
        return;
      }
      names = await backupFileContents(path);
      selection = [];
      source = path;
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  /** Restaure les coffres cochés un par un ; sur une erreur, ceux déjà restaurés quittent la liste. */
  async function restore(from: string) {
    problem = null;
    busy = true;
    const restored: { name: string; target: string }[] = [];
    try {
      for (const name of names.filter((candidate) => selection.includes(candidate))) {
        restored.push({ name, target: await restoreFromFile(from, name) });
      }
      const [only] = restored;
      onRestored(restored.length === 1 && only ? texts.restored(only.name, only.target) : texts.restoredMany(restored));
    } catch (error) {
      const done = restored.map(({ name }) => name);
      names = names.filter((name) => !done.includes(name));
      selection = selection.filter((name) => !done.includes(name));
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel" aria-labelledby="restore-title">
  <h2 id="restore-title">{texts.restoreTitle}</h2>
  <p class="explanation">{texts.restoreExplanation}</p>
  {#if source}
    {#if sourceName}
      <p class="file">{sourceName}</p>
    {/if}
    <VaultChoice {names} label={texts.vaultsToRestore} bind:selection />
  {/if}
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onCancel}>{texts.cancel}</button>
    <button type="button" class:primary={source === null} onclick={choose}>{texts.chooseBackupFile}</button>
  </div>
  {#if source}
    {@const from = source}
    <button
      type="button"
      class="primary confirm"
      disabled={selection.length === 0 || busy}
      onclick={() => restore(from)}
    >
      {busy ? texts.restoring : texts.restoreSelection(selection.length)}
    </button>
  {/if}
</section>

<style>
  .explanation {
    color: var(--text-muted);
  }

  .file {
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }

  .confirm {
    width: 100%;
    margin-top: var(--space-s);
  }
</style>
