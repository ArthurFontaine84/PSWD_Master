<script lang="ts">
  import { onDestroy } from "svelte";
  import { copyPassword, entryDetail, revealPassword, type EntryDetail } from "../lib/api";
  import { texts } from "../lib/i18n";

  interface Props {
    id: string;
    onBack: () => void;
    onError: (error: unknown) => void;
  }

  const { id, onBack, onError }: Props = $props();

  /** Un mot de passe affiché est masqué de nouveau au bout de ce délai (ms). */
  const REVEAL_DURATION = 30_000;

  let detail = $state<EntryDetail | null>(null);
  let revealed = $state<string | null>(null);
  let copied = $state<string | null>(null);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    entryDetail(id).then((loaded) => (detail = loaded), onError);
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
      copied = texts.copied(await copyPassword(id));
    } catch (error) {
      onError(error);
    }
  }

  // Le mot de passe affiché ne survit pas au changement d'entrée ni à la fermeture.
  onDestroy(hide);
</script>

<article aria-labelledby="entry-title">
  <button type="button" class="back" onclick={onBack}>{texts.back}</button>
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
    {#if copied}
      <p role="status">{copied}</p>
    {/if}
    <p class="meta">{texts.updated(detail.updatedAt)} · {texts.versions(detail.versions)}</p>
  {:else}
    <p>{texts.loading}</p>
  {/if}
</article>

<style>
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
</style>
