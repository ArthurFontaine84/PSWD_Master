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
    /** Compte de l'appareil. */
    accountId: string;
    onClose: () => void;
    /** Le compte de l'appareil a été supprimé : l'appareil n'est plus connecté. */
    onDisconnected: () => void;
  }

  const { address, publicAddress, accountId, onClose, onDisconnected }: Props = $props();

  const tabs = ["accounts", "vaults", "keys", "server"] as const;
  type Tab = (typeof tabs)[number];

  let tab = $state<Tab>("accounts");
  /** Une confirmation a son propre « Annuler » : le « Retour » du panneau est masqué. */
  let confirming = $state(false);

  function confirm(open: boolean) {
    confirming = open;
  }
</script>

<section class="panel admin" aria-labelledby="admin-title">
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
      <AdminAccounts {address} {accountId} onConfirming={confirm} {onDisconnected} />
    {:else if tab === "vaults"}
      <AdminVaults {accountId} onConfirming={confirm} />
    {:else if tab === "keys"}
      <AdminKeys />
    {:else}
      <AdminServer {publicAddress} />
    {/if}
  </div>
  {#if !confirming}
    <div class="actions">
      <button type="button" onclick={onClose}>{texts.back}</button>
    </div>
  {/if}
</section>

<style>
  /* Plus large que les autres cartes : les listes y tiennent sur deux colonnes. */
  .admin {
    max-width: 60rem;
  }

  .tabs {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(6rem, 1fr));
    gap: var(--space-s);
    margin-bottom: var(--space-m);
  }
</style>
