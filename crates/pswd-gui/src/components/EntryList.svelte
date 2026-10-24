<script lang="ts">
  import type { EntrySummary } from "../lib/api";
  import { texts } from "../lib/i18n";

  interface Props {
    entries: EntrySummary[];
    selected: string | null;
    onSelect: (id: string) => void;
  }

  const { entries, selected, onSelect }: Props = $props();
</script>

{#if entries.length === 0}
  <p class="empty">{texts.noEntry}</p>
{:else}
  <ul>
    {#each entries as entry (entry.id)}
      <li>
        <button
          type="button"
          class="entry"
          aria-current={entry.id === selected ? "true" : undefined}
          onclick={() => onSelect(entry.id)}
        >
          <span class="title">{entry.title}</span>
          {#if entry.username}
            <span class="username">{entry.username}</span>
          {/if}
          {#if entry.tags.length > 0}
            <span class="tags">{entry.tags.map((tag) => `#${tag}`).join(" ")}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .entry {
    display: grid;
    width: 100%;
    text-align: left;
    gap: 0.125rem;
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: none;
  }

  .entry:hover,
  .entry:focus-visible,
  .entry[aria-current="true"] {
    background: var(--surface);
  }

  .title {
    font-weight: 600;
  }

  .username,
  .tags,
  .empty {
    color: var(--text-muted);
  }

  .tags {
    font-size: 0.875rem;
  }
</style>
