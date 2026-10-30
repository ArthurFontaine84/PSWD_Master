<script lang="ts">
  import { serverVaults, type AdminVault as Vault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import AdminVault from "./AdminVault.svelte";

  let vaults = $state<Vault[] | null>(null);
  let problem = $state<string | null>(null);
  /** Identifiant du coffre ouvert dans le volet de gestion. */
  let selected = $state<string | null>(null);
  const shown = $derived(vaults?.find(({ id }) => id === selected));

  async function load() {
    try {
      vaults = await serverVaults();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  void load();
</script>

{#if shown !== undefined}
  {#key shown}
    <AdminVault vault={shown} onChanged={load} onClose={() => (selected = null)} />
  {/key}
{:else}
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  {#if vaults === null}
    {#if problem === null}
      <p>{texts.loading}</p>
    {/if}
  {:else if vaults.length === 0}
    <p class="muted">{texts.noServerVault}</p>
  {:else}
    <ul aria-label={texts.adminTabs.vaults}>
      {#each vaults as vault (vault.id)}
        <li>
          <button type="button" class="vault" aria-label={texts.manageVault(vault.name)} onclick={() => (selected = vault.id)}>
            <strong>{vault.name}</strong>
            <small>
              {vault.shared ? texts.shared : texts.privateVault} ·
              {vault.version === null ? texts.neverPublished : texts.version(vault.version)}
            </small>
            <small>
              {vault.members.length === 0 ? texts.noMember : vault.members.map(({ name }) => name).join(", ")}
            </small>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  ul {
    display: grid;
    gap: var(--space-s);
    padding: 0;
    list-style: none;
  }

  .vault {
    display: grid;
    width: 100%;
    gap: 0.25rem;
    text-align: left;
    overflow-wrap: anywhere;
  }

  .muted {
    color: var(--text-muted);
  }
</style>
