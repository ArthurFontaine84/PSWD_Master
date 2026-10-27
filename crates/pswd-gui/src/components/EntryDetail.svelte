<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    copyPassword,
    deleteEntry,
    entryDetail,
    entryHistory,
    revealPassword,
    type EntryDetail,
    type HistoryItem,
  } from "../lib/api";
  import { texts } from "../lib/i18n";
  import { showToast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    id: string;
    onBack: () => void;
    onEdit: (detail: EntryDetail) => void;
    onDeleted: () => void;
    onError: (error: unknown) => void;
  }

  const { id, onBack, onEdit, onDeleted, onError }: Props = $props();

  /** Un mot de passe affiché est masqué de nouveau au bout de ce délai (ms). */
  const REVEAL_DURATION = 30_000;

  let detail = $state<EntryDetail | null>(null);
  let revealed = $state<string | null>(null);
  let confirming = $state(false);
  let history = $state<HistoryItem[] | null>(null);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    entryDetail(id).then((loaded) => {
      detail = loaded;
      if (loaded.versions > 0) {
        entryHistory(id).then((items) => (history = items), onError);
      }
    }, onError);
  });

  function hide() {
    clearTimeout(hideTimer);
    revealed = null;
  }

  async function toggle() {
    if (revealed !== null) {
      hide();
      return;
    }
    try {
      revealed = await revealPassword(id);
      hideTimer = setTimeout(hide, REVEAL_DURATION);
    } catch (error) {
      onError(error);
    }
  }

  async function copy() {
    try {
      showToast(texts.copied(await copyPassword(id)));
    } catch (error) {
      onError(error);
    }
  }

  async function remove() {
    try {
      await deleteEntry(id);
      onDeleted();
    } catch (error) {
      onError(error);
    }
  }

  // Le mot de passe affiché ne survit pas au changement d'entrée ni à la fermeture.
  onDestroy(hide);
</script>

<article aria-labelledby="entry-title">
  <button type="button" class="back icon-button" aria-label={texts.back} title={texts.back} onclick={onBack}>
    <Icon name="back" />
  </button>
  {#if detail}
    <header>
      <h3 id="entry-title">{detail.title}</h3>
      <span class="kind">{texts.kinds[detail.kind]}</span>
    </header>
    <dl>
      {#if detail.username}
        <dt>{texts.username}</dt>
        <dd>{detail.username}</dd>
      {/if}
      {#if detail.hasPassword}
        <dt>{texts.password}</dt>
        <dd class="password">
          <code>{revealed ?? texts.mask}</code>
          <button type="button" onclick={toggle}>{revealed === null ? texts.show : texts.hide}</button>
          <button type="button" class="primary" onclick={copy}>{texts.copy}</button>
        </dd>
      {/if}
      {#if detail.urls.length > 0}
        <dt>{texts.urls}</dt>
        {#each detail.urls as url (url)}
          <dd>{url}</dd>
        {/each}
      {/if}
      {#if detail.notes}
        <dt>{texts.notes}</dt>
        <dd class="notes">{detail.notes}</dd>
      {/if}
      {#if detail.tags.length > 0}
        <dt>{texts.tags}</dt>
        <dd>{detail.tags.join(", ")}</dd>
      {/if}
    </dl>
    <p class="meta">{texts.updated(detail.updatedAt)} · {texts.versions(detail.versions)}</p>
    {@const shown = detail}
    {#if confirming}
      <div class="confirm" role="group" aria-label={texts.delete}>
        <p>{texts.confirmDelete(shown.title)}</p>
        <div class="actions">
          <button type="button" onclick={() => (confirming = false)}>{texts.cancel}</button>
          <button type="button" class="danger" onclick={remove}>{texts.delete}</button>
        </div>
      </div>
    {:else}
      <div class="actions">
        <button type="button" onclick={() => (confirming = true)}>{texts.delete}</button>
        <button type="button" class="primary" onclick={() => onEdit(shown)}>{texts.edit}</button>
      </div>
    {/if}
    {#if history !== null && history.length > 0}
      <section class="history" aria-labelledby="history-title">
        <h4 id="history-title">{texts.history}</h4>
        <ol aria-label={texts.history}>
          {#each history as version (version.updatedAt)}
            <li>
              <span class="date">{texts.updated(version.updatedAt)}</span>
              <span class="version">
                {version.title}{version.username ? ` — ${version.username}` : ""}
                {#if version.passwordChanged}
                  <span class="changed">{texts.passwordChanged}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ol>
      </section>
    {/if}
  {:else}
    <p>{texts.loading}</p>
  {/if}
</article>

<style>
  .back {
    margin-bottom: var(--space-s);
  }

  header {
    display: flex;
    align-items: baseline;
    gap: var(--space-s);
  }

  h3 {
    margin: var(--space-s) 0;
  }

  .kind,
  .meta,
  dt {
    color: var(--text-muted);
  }

  dl {
    display: grid;
    gap: 0.25rem;
  }

  dt {
    font-size: 0.875rem;
    margin-top: var(--space-s);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .password {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-s);
  }

  .password code {
    flex: 1;
    font-size: 1rem;
  }

  .notes {
    white-space: pre-wrap;
  }

  .meta {
    font-size: 0.875rem;
  }

  .confirm {
    padding: var(--space-m);
    border: 1px solid var(--danger);
    border-radius: var(--radius);
  }

  /* Historique : une frise verticale, du plus récent au plus ancien. */
  .history {
    margin-top: var(--space-l);
    padding-top: var(--space-m);
    border-top: 1px solid var(--border);
  }

  h4 {
    margin: 0 0 var(--space-m);
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  ol {
    display: grid;
    gap: var(--space-m);
    margin: 0;
    padding: 0 0 0 1.5rem;
    list-style: none;
    border-left: 2px solid var(--accent-soft);
  }

  li {
    position: relative;
    display: grid;
    gap: 0.125rem;
  }

  li::before {
    content: "";
    position: absolute;
    top: 0.3rem;
    left: calc(-1.5rem - 6px);
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    box-shadow: 0 0 0 3px var(--surface-solid);
  }

  .date {
    font-size: 0.8125rem;
    color: var(--text-muted);
  }

  .version {
    overflow-wrap: anywhere;
  }

  .changed {
    display: inline-block;
    margin-left: var(--space-s);
    padding: 0 0.5rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 600;
  }
</style>
