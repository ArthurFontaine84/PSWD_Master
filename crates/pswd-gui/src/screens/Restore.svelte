<script lang="ts">
  import { backupFileContents, restoreFromFile } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupFile } from "../lib/files";
  import { texts } from "../lib/i18n";

  interface Props {
    /** Restauration réussie : le message à afficher dans la liste des coffres. */
    onRestored: (notice: string) => void;
    onCancel: () => void;
  }

  const { onRestored, onCancel }: Props = $props();

  /** Le fichier `.pswd` choisi. */
  let source = $state<string | null>(null);
  let names = $state<string[]>([]);
  let problem = $state<string | null>(null);

  async function choose() {
    problem = null;
    try {
      const path = await pickBackupFile(texts.restoreFileTitle);
      if (path === null) {
        return;
      }
      names = await backupFileContents(path);
      source = path;
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function restore(name: string) {
    if (source === null) {
      return;
    }
    try {
      onRestored(texts.restored(name, await restoreFromFile(source, name)));
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<section aria-labelledby="restore-title">
  <h2 id="restore-title">{texts.restore}</h2>
  <p>{texts.restoreExplanation}</p>
  {#if source}
    <p class="folder">{source}</p>
    <ul>
      {#each names as name (name)}
        <li><button type="button" onclick={() => restore(name)}>{name}</button></li>
      {/each}
    </ul>
  {/if}
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onCancel}>{texts.cancel}</button>
    <button type="button" class="primary" onclick={choose}>{texts.chooseBackupFile}</button>
  </div>
</section>

<style>
  ul {
    list-style: none;
    padding: 0;
    display: grid;
    gap: var(--space-s);
  }

  .folder {
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
</style>
