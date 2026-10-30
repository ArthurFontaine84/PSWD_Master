<script lang="ts">
  import AdminAccounts from "../components/AdminAccounts.svelte";
  import AdminKeys from "../components/AdminKeys.svelte";
  import AdminServer from "../components/AdminServer.svelte";
  import AdminVaults from "../components/AdminVaults.svelte";
  import { texts } from "../lib/i18n";

  interface Props {
    /** Adresse locale du serveur. */
    address: string;
    publicAddress: string | null;
    onClose: () => void;
  }

  const { address, publicAddress, onClose }: Props = $props();

  const tabs = ["accounts", "vaults", "keys", "server"] as const;
  type Tab = (typeof tabs)[number];

  let tab = $state<Tab>("accounts");
</script>

<section class="panel" aria-labelledby="admin-title">
  <h2 id="admin-title">{texts.adminTitle}</h2>
  <div class="tabs" role="tablist" aria-label={texts.adminTitle}>
    {#each tabs as name (name)}
      <button
        type="button"
        role="tab"
        aria-selected={tab === name}
        class:primary={tab === name}
        onclick={() => (tab = name)}
      >
        {texts.adminTabs[name]}
      </button>
    {/each}
  </div>
  <div role="tabpanel" aria-label={texts.adminTabs[tab]}>
    {#if tab === "accounts"}
      <AdminAccounts {address} />
    {:else if tab === "vaults"}
      <AdminVaults />
    {:else if tab === "keys"}
      <AdminKeys />
    {:else}
      <AdminServer {publicAddress} />
    {/if}
  </div>
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.back}</button>
  </div>
</section>

<style>
  .tabs {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(6rem, 1fr));
    gap: var(--space-s);
    margin-bottom: var(--space-m);
  }
</style>
