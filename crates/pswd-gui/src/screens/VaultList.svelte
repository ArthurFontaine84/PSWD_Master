<script lang="ts">
  import {
    backupFileName,
    backupToFile,
    backupVaults,
    deviceKind,
    listVaults,
    type DeviceKind,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickBackupDestination, pickFolder } from "../lib/files";
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

  /** Inconnu tant que le backend n'a pas répondu : les dossiers restent cachés. */
  let device = $state<DeviceKind | null>(null);
  deviceKind().then(
    (kind) => (device = kind),
    () => undefined,
  );

  let backupResult = $state<string | null>(null);
  let backupProblem = $state<string | null>(null);

  async function backup() {
    backupResult = null;
    backupProblem = null;
    try {
      const destination = await pickFolder(texts.backupTitle);
      if (destination === null) {
        return;
      }
      const { folder, count } = await backupVaults(destination);
      backupResult = texts.backedUp(count, folder);
    } catch (error) {
      backupProblem = errorMessage(error);
    }
  }

  async function backupFile() {
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
        {#if device === "desktop"}
          <button type="button" onclick={backup}>{texts.backup}</button>
        {/if}
        <button type="button" onclick={backupFile}>{texts.backupToFile}</button>
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

  .vault:hover,
  .vault:focus-visible {
    background: var(--surface);
  }

  .actions {
    flex-wrap: wrap;
  }
</style>
