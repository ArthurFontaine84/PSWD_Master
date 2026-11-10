<script lang="ts">
  import {
    adminAccounts,
    attachAccount,
    deleteServerVault,
    removeMember,
    restoreVersion,
    vaultVersions,
    type AdminAccount,
    type AdminVault,
    type VaultVersion,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    vault: AdminVault;
    /** Compte de l'appareil : il quitte le coffre au lieu de s'en retirer. */
    accountId: string;
    /** Le coffre a changé : la liste est à relire. */
    onChanged: () => void;
    /** La confirmation de la suppression s'ouvre (`true`) ou se ferme. */
    onConfirming: (open: boolean) => void;
    onClose: () => void;
  }

  const { vault, accountId, onChanged, onConfirming, onClose }: Props = $props();

  let versions = $state<VaultVersion[] | null>(null);
  let accounts = $state<AdminAccount[]>([]);
  let chosen = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);
  let deleting = $state(false);
  let confirmation = $state("");

  $effect(() => {
    onConfirming(deleting);
    return () => onConfirming(false);
  });

  async function load() {
    try {
      [versions, accounts] = await Promise.all([vaultVersions(vault.id), adminAccounts()]);
      chosen = accounts[0]?.id ?? "";
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  void load();

  /** Une opération à la fois ; `done` dit si le volet se ferme ensuite. */
  async function run(action: () => Promise<string>, done = false) {
    busy = true;
    problem = null;
    try {
      showToast(await action());
      onChanged();
      if (done) {
        onClose();
      }
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function remove(id: string, name: string) {
    await run(async () => {
      await removeMember(vault.id, id);
      return texts.memberRemoved(name, vault.name);
    });
  }

  async function attach(event: SubmitEvent) {
    event.preventDefault();
    const account = accounts.find(({ id }) => id === chosen);
    if (account === undefined) {
      return;
    }
    await run(async () => {
      await attachAccount(vault.id, account.id);
      return texts.attached(account.name, vault.name);
    });
  }

  async function restore(version: number) {
    await run(async () => texts.versionRestored((await restoreVersion(vault.id, version)).name));
  }

  async function erase(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => {
      await deleteServerVault(vault.id, vault.name, confirmation);
      return texts.serverVaultDeleted(vault.name);
    }, true);
  }
</script>

<h3>{vault.name}</h3>
{#if problem}
  <p role="alert">{problem}</p>
{/if}
{#if deleting}
  <form onsubmit={erase}>
    <p>{texts.deleteServerVaultExplanation(vault.name)}</p>
    <label>
      {texts.vaultName}
      <input bind:value={confirmation} {...verbatim} />
    </label>
    <div class="actions">
      <button type="button" onclick={() => (deleting = false)}>{texts.cancel}</button>
      <button type="submit" class="danger" disabled={busy || confirmation.trim() !== vault.name}>
        {texts.deleteForever}
      </button>
    </div>
  </form>
{:else}
  <h4>{texts.members}</h4>
  {#if vault.members.length === 0}
    <p class="muted">{texts.attachExplanation}</p>
    {#if accounts.length > 0}
      <form onsubmit={attach}>
        <label>
          {texts.attachAccount}
          <select bind:value={chosen}>
            {#each accounts as account (account.id)}
              <option value={account.id}>{account.name}</option>
            {/each}
          </select>
        </label>
        <div class="actions">
          <button type="submit" class="primary" disabled={busy}>{texts.attach}</button>
        </div>
      </form>
    {/if}
  {:else}
    <ul aria-label={texts.members}>
      {#each vault.members as member (member.id)}
        <li>
          <span>{member.name}</span>
          <!-- Le propriétaire d'un coffre privé reste ; son propre compte quitte le coffre (#12). -->
          {#if vault.shared && member.id !== accountId}
            <button
              type="button"
              disabled={busy}
              aria-label={texts.removeMember(member.name)}
              onclick={() => remove(member.id, member.name)}
            >
              {texts.remove}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <h4>{texts.versionsTitle}</h4>
  {#if versions === null}
    {#if problem === null}
      <p>{texts.loading}</p>
    {/if}
  {:else if versions.length === 0}
    <p class="muted">{texts.noVersion}</p>
  {:else}
    <ul aria-label={texts.versionsTitle}>
      {#each versions as version (version.version)}
        <li>
          <span>{texts.versionLine(version.version, version.createdAt, version.size)}</span>
          {#if version.version === vault.version}
            <span class="muted">{texts.currentVersion}</span>
          {:else}
            <button
              type="button"
              disabled={busy}
              aria-label={texts.restoreVersion(version.version)}
              onclick={() => restore(version.version)}
            >
              {texts.restore}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.allVaults}</button>
    <button type="button" class="danger" disabled={busy} onclick={() => (deleting = true)}>
      {texts.deleteServerVault}
    </button>
  </div>
{/if}

<style>
  h3 {
    overflow-wrap: anywhere;
  }

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
    margin-bottom: var(--space-s);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
