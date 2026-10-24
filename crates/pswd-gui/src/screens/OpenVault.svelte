<script lang="ts">
  import EntryDetail from "../components/EntryDetail.svelte";
  import EntryList from "../components/EntryList.svelte";
  import {
    listEntries,
    listTags,
    lockVault,
    type EntrySummary,
    type TagCount,
    type VaultInfo,
  } from "../lib/api";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";

  interface Props {
    vault: VaultInfo;
    onLocked: () => void;
    /** Le coffre s'est verrouillé pendant une opération. */
    onExpired: () => void;
  }

  const { vault, onLocked, onExpired }: Props = $props();

  let text = $state("");
  let tag = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let entries = $state<EntrySummary[]>([]);
  let tags = $state<TagCount[]>([]);
  let problem = $state<string | null>(null);
  let searchField: HTMLInputElement | undefined = $state();

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

  $effect(() => {
    void search(text, tag);
  });

  listTags().then((loaded) => (tags = loaded), fail);

  async function lock() {
    await lockVault();
    onLocked();
  }

  function shortcut(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      searchField?.focus();
    }
  }
</script>

<svelte:window onkeydown={shortcut} />

<section class="vault" class:has-selection={selected !== null} aria-labelledby="vault-title">
  <header>
    <h2 id="vault-title">{texts.vaultTitle(vault.name)}</h2>
    <button type="button" onclick={lock}>{texts.lock}</button>
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
          spellcheck="false"
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
      <EntryList {entries} {selected} onSelect={(id) => (selected = id)} />
    </div>
    <div class="detail-pane">
      {#if selected}
        {#key selected}
          <EntryDetail id={selected} onBack={() => (selected = null)} onError={fail} />
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
    gap: var(--space-m);
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
