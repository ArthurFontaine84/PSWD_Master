<script lang="ts">
  import { listVaults } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";

  interface Props {
    notice?: string;
    onOpen: (name: string) => void;
    onCreate: () => void;
  }

  const { notice, onOpen, onCreate }: Props = $props();
  const vaults = listVaults();
</script>

<section aria-labelledby="vaults-title">
  <h2 id="vaults-title">{texts.vaults}</h2>
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
  {:catch error}
    <p role="alert">{errorMessage(error)}</p>
  {/await}
  <button type="button" class="primary" onclick={onCreate}>{texts.newVault}</button>
</section>

<style>
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
</style>
