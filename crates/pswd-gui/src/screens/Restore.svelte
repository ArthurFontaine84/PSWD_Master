<script lang="ts">
  import {
    backupContents,
    backupFileContents,
    deviceKind,
    restoreFromFile,
    restoreVault,
    type DeviceKind,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupFile, pickFolder } from "../lib/files";
  import { texts } from "../lib/i18n";

  interface Props {
    /** Restauration réussie : le message à afficher dans la liste des coffres. */
    onRestored: (notice: string) => void;
    onCancel: () => void;
  }

  const { onRestored, onCancel }: Props = $props();

  /** Une sauvegarde datée : un dossier, ou un fichier `.pswd`. */
  interface Source {
    kind: "folder" | "file";
    path: string;
  }

  /** Inconnu tant que le backend n'a pas répondu : les dossiers restent cachés. */
  let device = $state<DeviceKind | null>(null);
  deviceKind().then(
    (kind) => (device = kind),
    () => undefined,
  );

  let source = $state<Source | null>(null);
  let names = $state<string[]>([]);
  let problem = $state<string | null>(null);

  async function choose(kind: Source["kind"]) {
    problem = null;
    try {
      const path =
        kind === "folder"
          ? await pickFolder(texts.restoreTitle)
          : await pickBackupFile(texts.restoreFileTitle);
      if (path === null) {
        return;
      }
      names = kind === "folder" ? await backupContents(path) : await backupFileContents(path);
      source = { kind, path };
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function restore(name: string) {
    if (source === null) {
      return;
    }
    try {
      const target =
        source.kind === "folder"
          ? await restoreVault(source.path, name)
          : await restoreFromFile(source.path, name);
      onRestored(texts.restored(name, target));
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<section aria-labelledby="restore-title">
  <h2 id="restore-title">{texts.restore}</h2>
  <p>{texts.restoreExplanation}</p>
  {#if source}
    <p class="folder">{source.path}</p>
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
    {#if device === "desktop"}
      <button type="button" onclick={() => choose("folder")}>{texts.chooseBackupFolder}</button>
    {/if}
    <button type="button" class="primary" onclick={() => choose("file")}>
      {texts.chooseBackupFile}
    </button>
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
