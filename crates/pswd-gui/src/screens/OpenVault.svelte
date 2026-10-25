<script lang="ts">
  import { onMount } from "svelte";
  import ChangePassword from "../components/ChangePassword.svelte";
  import DeleteVault from "../components/DeleteVault.svelte";
  import EntryDetailView from "../components/EntryDetail.svelte";
  import EntryForm from "../components/EntryForm.svelte";
  import EntryList from "../components/EntryList.svelte";
  import ImportEntries from "../components/ImportEntries.svelte";
  import {
    addEntry,
    listEntries,
    listTags,
    lockVault,
    updateEntry,
    type EntryDetail,
    type EntryInput,
    type EntrySummary,
    type TagCount,
    type VaultInfo,
  } from "../lib/api";
  import { onBack } from "../lib/back";
  import { emptyForm, formFrom } from "../lib/entryForm";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";

  interface Props {
    vault: VaultInfo;
    onLocked: () => void;
    /** Le coffre s'est verrouillé pendant une opération. */
    onExpired: () => void;
    onDeleted: () => void;
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
  /** Action de gestion ouverte à la place du détail. */
  let panel = $state<"password" | "import" | "delete" | null>(null);

  /** Une seule chose à la fois à la place du détail. */
  function show(next: "password" | "import" | "delete") {
    editor = null;
    panel = next;
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
    if (panel !== null) {
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

  onMount(() => onBack(back));

  function shortcut(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      searchField?.focus();
    }
  }
</script>

<svelte:window onkeydown={shortcut} />

<section class="vault" class:has-selection={selected !== null || editor !== null || panel !== null} aria-labelledby="vault-title">
  <header>
    <h2 id="vault-title">{texts.vaultTitle(vault.name)}</h2>
    <div class="actions">
      <button type="button" class="primary" onclick={startEntry}>{texts.newEntry}</button>
      <button type="button" aria-expanded={managing} onclick={() => (managing = !managing)}>
        {texts.manage}
      </button>
      <button type="button" onclick={lock}>{texts.lock}</button>
    </div>
  </header>
  {#if managing}
    <div class="actions manage">
      <button type="button" onclick={() => show("password")}>{texts.changePassword}</button>
      <button type="button" onclick={() => show("import")}>{texts.importCsv}</button>
      <button type="button" class="danger" onclick={() => show("delete")}>{texts.deleteVault}</button>
    </div>
  {/if}
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
      {#if problem}
        <p role="alert">{problem}</p>
      {/if}
      <p class="count">{texts.entryCount(entries.length)}</p>
      <EntryList {entries} {selected} onSelect={select} />
    </div>
    <div class="detail-pane">
      {#if panel === "password"}
        <ChangePassword onClose={() => (panel = null)} {onExpired} />
      {:else if panel === "import"}
        <ImportEntries onImported={() => (revision += 1)} onClose={() => (panel = null)} {onExpired} />
      {:else if panel === "delete"}
        <DeleteVault name={vault.name} {onDeleted} onClose={() => (panel = null)} />
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
  </div>
</section>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-m);
  }

  .manage {
    justify-content: flex-start;
    margin-bottom: var(--space-m);
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

  /* Écran étroit : la liste, ou le détail en plein écran. */
  .has-selection .list-pane,
  .vault:not(.has-selection) .detail-pane {
    display: none;
  }

  /* Écran large : la liste à gauche, le détail à droite. */
  @media (min-width: 48rem) {
    .layout {
      display: grid;
      grid-template-columns: minmax(16rem, 1fr) 2fr;
      gap: var(--space-l);
    }

    .has-selection .list-pane,
    .vault:not(.has-selection) .detail-pane {
      display: block;
    }

    .layout :global(.back) {
      display: none;
    }
  }
</style>
