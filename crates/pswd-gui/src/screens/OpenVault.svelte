<script lang="ts">
  import { lockVault, type VaultInfo } from "../lib/api";
  import { texts } from "../lib/i18n";

  interface Props {
    vault: VaultInfo;
    onLocked: () => void;
  }

  const { vault, onLocked }: Props = $props();

  async function lock() {
    await lockVault();
    onLocked();
  }
</script>

<section aria-labelledby="vault-title">
  <header>
    <h2 id="vault-title">{texts.vaultTitle(vault.name)}</h2>
    <button type="button" onclick={lock}>{texts.lock}</button>
  </header>
  <p>{texts.entryCount(vault.entries)}</p>
</section>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-m);
  }
</style>
