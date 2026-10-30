<script lang="ts">
  import { cancelKey, pendingKeys, type PendingKey } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { showToast } from "../lib/toast.svelte";

  let keys = $state<PendingKey[] | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function load() {
    try {
      keys = await pendingKeys();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  void load();

  async function cancel(id: string) {
    busy = true;
    problem = null;
    try {
      await cancelKey(id);
      showToast(texts.keyCancelled);
      await load();
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

{#if problem}
  <p role="alert">{problem}</p>
{/if}
{#if keys === null}
  {#if problem === null}
    <p>{texts.loading}</p>
  {/if}
{:else if keys.length === 0}
  <p class="muted">{texts.noPendingKey}</p>
{:else}
  <ul aria-label={texts.adminTabs.keys}>
    {#each keys as key (key.id)}
      <li>
        <span>
          {texts.keyPurpose(key.purpose, key.name ?? "")}
          <small>{texts.expires(key.expiresAt)}</small>
        </span>
        <button type="button" disabled={busy} onclick={() => cancel(key.id)}>{texts.cancelKey}</button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul {
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-s);
    margin-bottom: var(--space-m);
  }

  small {
    display: block;
  }

  .muted {
    color: var(--text-muted);
  }
</style>
