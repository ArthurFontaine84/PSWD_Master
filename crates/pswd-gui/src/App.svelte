<script lang="ts">
  import { listVaults } from "./lib/api";
  import { errorMessage } from "./lib/errors";
  import { texts } from "./lib/i18n";

  const vaults = listVaults();
</script>

<main>
  <h1>{texts.appTitle}</h1>
  <section aria-labelledby="vaults-title">
    <h2 id="vaults-title">{texts.vaults}</h2>
    {#await vaults}
      <p>{texts.loading}</p>
    {:then names}
      {#if names.length === 0}
        <p>{texts.noVault}</p>
      {:else}
        <ul>
          {#each names as name (name)}
            <li>{name}</li>
          {/each}
        </ul>
      {/if}
    {:catch error}
      <p role="alert">{errorMessage(error)}</p>
    {/await}
  </section>
</main>

<style>
  main {
    max-width: 40rem;
    margin: 0 auto;
    padding: var(--space-l) var(--space-m);
  }

  ul {
    list-style: none;
    padding: 0;
  }

  li {
    padding: var(--space-s) var(--space-m);
    border-bottom: 1px solid var(--border);
  }

  [role="alert"] {
    color: var(--danger);
  }
</style>
