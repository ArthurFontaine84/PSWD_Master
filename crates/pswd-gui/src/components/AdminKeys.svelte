<script lang="ts">
  import { onMount } from "svelte";
  import { cancelKey, pendingKeys, type PendingKey } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { KEY_LIST } from "../lib/layout";
  import { everyExchange } from "../lib/poll";
  import { showToast } from "../lib/toast.svelte";
  import Confirm from "./Confirm.svelte";
  import Paged from "./Paged.svelte";

  interface Props {
    /** La confirmation d'une annulation s'ouvre (`true`) ou se ferme. */
    onConfirming: (open: boolean) => void;
  }

  const { onConfirming }: Props = $props();

  let keys = $state<PendingKey[] | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  /** Clé dont l'annulation attend confirmation. */
  let asking = $state<PendingKey | null>(null);
  let page = $state(0);

  $effect(() => {
    onConfirming(asking !== null);
    return () => onConfirming(false);
  });

  /** `quiet` : relecture automatique, dont l'échec n'est pas affiché. */
  async function load(quiet = false) {
    try {
      keys = await pendingKeys();
    } catch (error) {
      if (!quiet) {
        problem = errorMessage(error);
      }
    }
  }

  void load();

  // Une clé utilisée depuis un autre appareil disparaît de la liste.
  onMount(() =>
    everyExchange(() => {
      if (!busy && asking === null) {
        void load(true);
      }
    }),
  );

  function label(key: PendingKey): string {
    return texts.keyPurpose(key.purpose, key.name ?? "");
  }

  async function cancel(key: PendingKey) {
    busy = true;
    problem = null;
    try {
      await cancelKey(key.id);
      asking = null;
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
{#if asking !== null}
  {@const key = asking}
  <Confirm
    title={texts.cancelKey}
    message={texts.cancelKeyExplanation(label(key))}
    action={texts.cancelKey}
    dismiss={texts.keepKey}
    {busy}
    onConfirm={() => cancel(key)}
    onCancel={() => (asking = null)}
  />
{:else if keys === null}
  {#if problem === null}
    <p>{texts.loading}</p>
  {/if}
{:else if keys.length === 0}
  <p class="muted">{texts.noPendingKey}</p>
{:else}
  <Paged items={keys} layout={KEY_LIST} label={texts.keyPages} bind:page>
    {#snippet children(visible)}
      <ul aria-label={texts.adminTabs.keys}>
        {#each visible as key (key.id)}
          <li>
            <span>
              {label(key)}
              <small>{texts.expires(key.expiresAt)}</small>
            </span>
            <button
              type="button"
              disabled={busy}
              onclick={() => {
                problem = null;
                asking = key;
              }}
            >
              {texts.cancelKey}
            </button>
          </li>
        {/each}
      </ul>
    {/snippet}
  </Paged>
{/if}

<style>
  ul {
    padding: 0;
    margin: 0;
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
