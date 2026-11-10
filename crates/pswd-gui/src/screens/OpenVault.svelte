<script lang="ts">
  import { onMount } from "svelte";
  import { fly, scale } from "svelte/transition";
  import ChangePassword from "../components/ChangePassword.svelte";
  import DeleteVault from "../components/DeleteVault.svelte";
  import EntryDetailView from "../components/EntryDetail.svelte";
  import EntryForm from "../components/EntryForm.svelte";
  import EntryList from "../components/EntryList.svelte";
  import Icon from "../components/Icon.svelte";
  import Paged from "../components/Paged.svelte";
  import ImportEntries from "../components/ImportEntries.svelte";
  import LeaveVault from "../components/LeaveVault.svelte";
  import Members from "../components/Members.svelte";
  import {
    addEntry,
    checkVault,
    listEntries,
    listTags,
    lockVault,
    onVaultSynced,
    serverStatus,
    syncVault,
    updateEntry,
    type EntryDetail,
    type EntryInput,
    type EntrySummary,
    type SyncReport,
    type TagCount,
    type VaultInfo,
  } from "../lib/api";
  import { onBack } from "../lib/back";
  import { emptyForm, formFrom } from "../lib/entryForm";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { ENTRY_LIST } from "../lib/layout";
  import { motion } from "../lib/motion";
  import { syncMessage, syncProblem } from "../lib/sync";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    vault: VaultInfo;
    onLocked: () => void;
    /** Le coffre s'est verrouillé pendant une opération. */
    onExpired: () => void;
    /** Le coffre a été supprimé ou retiré de l'appareil ; `message` le dit. */
    onDeleted: (message: string) => void;
  }

  const { vault, onLocked, onExpired, onDeleted }: Props = $props();

  let text = $state("");
  let tag = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let entries = $state<EntrySummary[]>([]);
  let tags = $state<TagCount[]>([]);
  let problem = $state<string | null>(null);
  let searchField: HTMLInputElement | undefined = $state();
  /** Formulaire ouvert à la place du détail : nouvelle entrée, ou modification. */
  let editor = $state<{ mode: "add" } | { mode: "edit"; detail: EntryDetail } | null>(null);
  /** Augmente après chaque modification : la liste et les tags sont relus. */
  let revision = $state(0);
  let managing = $state(false);
  let menuAnchor: HTMLElement | undefined = $state();
  type Panel = "password" | "import" | "delete" | "members" | "leave";
  /** Action de gestion ouverte à la place du détail. */
  let panel = $state<Panel | null>(null);
  let page = $state(0);
  /** Change dès que le volet de détail montre autre chose : son contenu arrive animé. */
  const paneKey = $derived(`${panel ?? ""}:${editor?.mode ?? ""}:${selected ?? ""}`);
  /** L'appareil est connecté à un serveur : le bouton « Synchroniser » est proposé. */
  let connected = $state(false);
  let syncing = $state(false);
  /** Dernier problème de synchro, effacé par la synchro suivante réussie. */
  let syncNotice = $state<string | null>(null);
  /** Coffre commun publié sur le serveur : ses membres se gèrent ici, et il se quitte au lieu
   * d'être supprimé (supprimé ici, il serait téléchargé de nouveau). */
  let sharing = $state(false);
  /** Coffre privé publié sur le serveur : supprimé partout, ou retiré de l'appareil. */
  let privateOnServer = $state(false);

  // Sans état lisible, le coffre reste utilisable en local, sans bouton de synchro.
  serverStatus().then(
    (status) => {
      connected = status.address !== null;
      const onServer: (string | null)[] = ["synced", "outdated", "needsUnlock"];
      const published = status.vaults.find(({ name, state }) => name === vault.name && onServer.includes(state));
      sharing = connected && published?.shared === true;
      privateOnServer = connected && published?.shared === false;
    },
    () => undefined,
  );

  /** Le coffre quitté vient d'être fermé et supprimé de l'appareil. */
  function left() {
    onLocked();
  }

  /** Fin d'une synchro : les modifications reçues du serveur sont affichées. */
  function synced(report: SyncReport) {
    if (report.vault !== vault.name) {
      return;
    }
    if (report.state === "left" || report.state === "deleted") {
      showToast(syncMessage(report));
      onLocked();
      return;
    }
    syncNotice = syncProblem(report);
    if (report.changed) {
      revision += 1;
    }
    if (report.passwordChanged) {
      showToast(texts.passwordChangedElsewhere);
    }
  }

  async function synchronize() {
    syncing = true;
    try {
      const report = await syncVault();
      synced(report);
      if (syncNotice === null && !report.passwordChanged) {
        showToast(syncMessage(report));
      }
    } catch (error) {
      fail(error);
    } finally {
      syncing = false;
    }
  }

  /** Une seule chose à la fois à la place du détail. */
  function show(next: Panel) {
    managing = false;
    editor = null;
    panel = next;
  }

  /** Un clic hors du menu de gestion le referme. */
  function closeMenuOutside(event: MouseEvent) {
    if (managing && !menuAnchor?.contains(event.target as Node)) {
      managing = false;
    }
  }

  function fail(error: unknown) {
    if (isVaultLocked(error)) {
      onExpired();
    } else {
      problem = errorMessage(error);
    }
  }

  async function search(text: string, tag: string | null) {
    try {
      entries = await listEntries(text, tag);
      problem = null;
    } catch (error) {
      fail(error);
    }
  }

  // Relancée à chaque frappe, changement de tag ou modification d'une entrée.
  $effect(() => {
    void revision;
    void search(text, tag);
  });

  // Une nouvelle recherche repart de la première page.
  $effect(() => {
    void text;
    void tag;
    page = 0;
  });

  $effect(() => {
    void revision;
    listTags().then((loaded) => (tags = loaded), fail);
  });

  function select(id: string) {
    editor = null;
    panel = null;
    selected = id;
  }

  function startEntry() {
    panel = null;
    editor = { mode: "add" };
  }

  /** Enregistre le formulaire, puis montre l'entrée enregistrée. */
  async function save(input: EntryInput) {
    const saved = editor?.mode === "edit" ? await updateEntry(editor.detail.id, input) : await addEntry(input);
    editor = null;
    selected = saved.id;
    revision += 1;
    showToast(texts.entrySaved);
  }

  function deleted() {
    selected = null;
    revision += 1;
  }

  async function lock() {
    await lockVault();
    onLocked();
  }

  // Bouton retour : referme ce qui est ouvert à la place du détail, puis verrouille le coffre.
  function back(): boolean {
    if (managing) {
      managing = false;
    } else if (panel !== null) {
      panel = null;
    } else if (editor !== null) {
      editor = null;
    } else if (selected !== null) {
      selected = null;
    } else {
      void lock();
    }
    return true;
  }

  onMount(() => {
    const stopBack = onBack(back);
    const stopSynced = onVaultSynced(synced);
    // Vérification muette (SPEC §7) : elle ne repousse pas le verrouillage.
    const checking = setInterval(() => {
      if (document.visibilityState === "visible") {
        checkVault().catch(() => undefined);
      }
    }, 30_000);
    return () => {
      clearInterval(checking);
      stopBack();
      void stopSynced.then((stop) => {
        stop();
      });
    };
  });

  function shortcut(event: KeyboardEvent) {
    if (event.key === "Escape") {
      managing = false;
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      searchField?.focus();
    }
  }
</script>

<svelte:window onkeydown={shortcut} onclick={closeMenuOutside} />

<section class="vault" class:has-selection={selected !== null || editor !== null || panel !== null} aria-labelledby="vault-title">
  <header>
    <h2 id="vault-title">{vault.name}</h2>
    <div class="tools">
      <button type="button" class="icon-button" aria-label={texts.newEntry} title={texts.newEntry} onclick={startEntry}>
        <Icon name="plus" />
      </button>
      {#if connected}
        <button
          type="button"
          class="icon-button"
          aria-label={texts.synchronize}
          title={texts.synchronize}
          disabled={syncing}
          onclick={synchronize}
        >
          <Icon name="sync" />
        </button>
      {/if}
      <div class="menu-anchor" bind:this={menuAnchor}>
        <button
          type="button"
          class="icon-button"
          aria-label={texts.manage}
          title={texts.manage}
          aria-haspopup="menu"
          aria-expanded={managing}
          onclick={() => (managing = !managing)}
        >
          <Icon name="settings" />
        </button>
        {#if managing}
          <div
            class="menu"
            role="menu"
            aria-label={texts.manage}
            transition:scale={{ start: 0.95, duration: motion(150) }}
          >
            <button type="button" role="menuitem" onclick={() => show("password")}>{texts.changePassword}</button>
            <button type="button" role="menuitem" onclick={() => show("import")}>{texts.importCsv}</button>
            {#if sharing}
              <button type="button" role="menuitem" onclick={() => show("members")}>{texts.members}</button>
              <button type="button" role="menuitem" class="danger-item" onclick={() => show("leave")}>
                {texts.leaveVault}
              </button>
            {:else}
              <button type="button" role="menuitem" class="danger-item" onclick={() => show("delete")}>
                {texts.deleteVault}
              </button>
            {/if}
          </div>
        {/if}
      </div>
      <button type="button" class="icon-button" aria-label={texts.lock} title={texts.lock} onclick={lock}>
        <Icon name="lock" />
      </button>
    </div>
  </header>
  <div class="layout">
    <div class="list-pane">
      <div class="filters">
        <input
          type="search"
          bind:this={searchField}
          bind:value={text}
          aria-label={texts.search}
          placeholder={texts.searchHint}
          {...verbatim}
        />
        {#if tags.length > 0}
          <select bind:value={tag} aria-label={texts.tagFilter}>
            <option value={null}>{texts.allTags}</option>
            {#each tags as { tag: name, count } (name)}
              <option value={name}>{name} ({count})</option>
            {/each}
          </select>
        {/if}
      </div>
      {#if syncNotice}
        <p role="status" class="notice">{syncNotice}</p>
      {/if}
      {#if problem}
        <p role="alert">{problem}</p>
      {/if}
      <p class="count">{texts.entryCount(entries.length)}</p>
      <Paged items={entries} layout={ENTRY_LIST} label={texts.pagination} bind:page>
        {#snippet children(visible)}
          <EntryList entries={visible} {selected} onSelect={select} />
        {/snippet}
      </Paged>
    </div>
    <div class="detail-pane">
      {#key paneKey}
        <div in:fly={{ x: 16, duration: motion(250) }}>
          {#if panel === "password"}
            <ChangePassword onClose={() => (panel = null)} {onExpired} />
          {:else if panel === "import"}
            <ImportEntries onImported={() => (revision += 1)} onClose={() => (panel = null)} {onExpired} />
          {:else if panel === "members"}
            <Members name={vault.name} onClose={() => (panel = null)} />
          {:else if panel === "leave"}
            <LeaveVault name={vault.name} onLeft={left} {onDeleted} onClose={() => (panel = null)} />
          {:else if panel === "delete"}
            <DeleteVault name={vault.name} onServer={privateOnServer} {onDeleted} onClose={() => (panel = null)} />
          {:else if editor}
            <h3>{editor.mode === "edit" ? texts.editEntry(editor.detail.title) : texts.newEntry}</h3>
            {#key editor}
              <EntryForm
                initial={editor.mode === "edit" ? formFrom(editor.detail) : emptyForm()}
                editing={editor.mode === "edit"}
                onSubmit={save}
                onCancel={() => (editor = null)}
                {onExpired}
              />
            {/key}
          {:else if selected}
            {#key `${selected}:${String(revision)}`}
              <EntryDetailView
                id={selected}
                onBack={() => (selected = null)}
                onEdit={(detail) => {
                  panel = null;
                  editor = { mode: "edit", detail };
                }}
                onDeleted={deleted}
                onError={fail}
              />
            {/key}
          {:else}
            <p class="hint">{texts.chooseEntry}</p>
          {/if}
        </div>
      {/key}
    </div>
  </div>
</section>

<style>
  header {
    display: grid;
    justify-items: center;
    gap: var(--space-m);
    margin-bottom: var(--space-l);
  }

  h2 {
    margin: 0;
    font-size: 1.375rem;
    text-align: center;
    overflow-wrap: anywhere;
  }

  .tools {
    display: flex;
    gap: var(--space-l);
  }

  .menu-anchor {
    position: relative;
  }

  .menu {
    position: absolute;
    top: calc(100% + var(--space-s));
    left: 50%;
    z-index: 10;
    display: grid;
    min-width: 16rem;
    padding: 0.375rem;
    transform: translateX(-50%);
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

  .menu .danger-item {
    color: var(--danger);
  }

  @media (hover: hover) {
    .menu .danger-item:hover {
      background: var(--danger-soft);
    }
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-s);
  }

  .filters input {
    flex: 1;
    min-width: 12rem;
  }

  .count,
  .hint {
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .detail-pane {
    align-self: start;
    padding: var(--space-m);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    -webkit-backdrop-filter: var(--blur);
    backdrop-filter: var(--blur);
  }

  /* Écran étroit : la liste, ou le détail en plein écran. */
  .has-selection .list-pane,
  .vault:not(.has-selection) .detail-pane {
    display: none;
  }

  /* Écran large : la liste à gauche, le détail à droite. */
  @media (min-width: 48rem) {
    h2 {
      font-size: 1.75rem;
    }

    .layout {
      display: grid;
      grid-template-columns: minmax(16rem, 1fr) 2fr;
      gap: var(--space-l);
    }

    .has-selection .list-pane,
    .vault:not(.has-selection) .detail-pane {
      display: block;
    }

    /* Le détail reste visible et défile seul s'il est plus haut que la fenêtre. */
    .detail-pane {
      position: sticky;
      top: var(--space-m);
      max-height: calc(100vh - 2 * var(--space-m));
      overflow-y: auto;
      padding: var(--space-l);
    }

    .layout :global(.back) {
      display: none;
    }
  }
</style>
