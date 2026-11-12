<script lang="ts">
  import { addMember, adminAccounts, deleteAccount, revokeDevice, setAdmin, type AdminAccount } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { ACCOUNT_LIST } from "../lib/layout";
  import { byName } from "../lib/search";
  import { showToast } from "../lib/toast.svelte";
  import Paged from "./Paged.svelte";

  interface Props {
    /** Adresse du serveur, à transmettre avec la clé d'un nouveau membre. */
    address: string;
    /** Compte de l'appareil : seul administrateur que l'on supprime d'ici, s'il n'est pas le dernier. */
    accountId: string;
    /** La confirmation d'une suppression s'ouvre (`true`) ou se ferme. */
    onConfirming: (open: boolean) => void;
    /** Le compte de l'appareil a été supprimé : l'appareil n'est plus connecté. */
    onDisconnected: () => void;
  }

  const { address, accountId, onConfirming, onDisconnected }: Props = $props();

  let accounts = $state<AdminAccount[] | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  let memberName = $state("");
  /** Clé d'un nouveau membre : montrée une fois, oubliée en la fermant. */
  let issued = $state<{ name: string; key: string } | null>(null);
  /** Compte dont la suppression attend que son nom soit retapé. */
  let deleting = $state<AdminAccount | null>(null);
  let confirmation = $state("");
  const admins = $derived(accounts?.filter(({ isAdmin }) => isAdmin).length ?? 0);
  let query = $state("");
  let page = $state(0);
  const found = $derived(byName(accounts ?? [], query));

  // Une nouvelle recherche repart de la première page.
  $effect(() => {
    void query;
    page = 0;
  });

  $effect(() => {
    onConfirming(deleting !== null);
    return () => onConfirming(false);
  });

  async function load() {
    try {
      accounts = await adminAccounts();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  void load();

  /** Une opération à la fois ; la liste est relue ensuite, si `reload` le permet. */
  async function run(action: () => Promise<string | null>, reload = () => true) {
    busy = true;
    problem = null;
    try {
      const message = await action();
      if (message !== null) {
        showToast(message);
      }
      if (reload()) {
        await load();
      }
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const name = memberName.trim();
    if (name === "") {
      problem = texts.validation.memberNameRequired;
      return;
    }
    await run(async () => {
      issued = { name, key: await addMember(name) };
      memberName = "";
      return null;
    });
  }

  async function toggleAdmin(account: AdminAccount) {
    await run(async () => {
      await setAdmin(account.id, !account.isAdmin);
      return account.isAdmin ? texts.roleRemoved(account.name) : texts.roleGiven(account.name);
    });
  }

  async function revoke(id: string, name: string) {
    await run(async () => {
      await revokeDevice(id);
      return texts.deviceRevoked(name);
    });
  }

  function startDeletion(account: AdminAccount) {
    problem = null;
    confirmation = "";
    deleting = account;
  }

  async function remove(event: SubmitEvent) {
    event.preventDefault();
    const account = deleting;
    if (account === null) {
      return;
    }
    // Le compte de l'appareil supprimé, il n'y a plus de liste à relire : l'écran se ferme.
    let disconnected = false;
    await run(
      async () => {
        disconnected = await deleteAccount(account.id, account.name, confirmation);
        deleting = null;
        return disconnected ? texts.ownAccountDeleted : texts.accountDeleted(account.name);
      },
      () => !disconnected,
    );
    if (disconnected) {
      onDisconnected();
    }
  }
</script>

{#if problem}
  <p role="alert">{problem}</p>
{/if}
{#if issued !== null}
  <h3>{texts.memberKeyTitle(issued.name)}</h3>
  <p>{texts.memberKeyExplanation(address)}</p>
  <p class="key"><code>{issued.key}</code></p>
  <div class="actions">
    <button type="button" class="primary" onclick={() => (issued = null)}>{texts.close}</button>
  </div>
{:else if deleting !== null}
  {@const own = deleting.id === accountId}
  <h3>{own ? texts.deleteOwnAccount : texts.deleteAccount}</h3>
  <form onsubmit={remove}>
    <p>{own ? texts.deleteOwnAccountExplanation : texts.deleteAccountExplanation(deleting.name)}</p>
    <label>
      {texts.accountNameConfirmation}
      <input bind:value={confirmation} {...verbatim} />
    </label>
    <div class="actions">
      <button type="button" onclick={() => (deleting = null)}>{texts.cancel}</button>
      <button type="submit" class="danger" disabled={busy || confirmation.trim() !== deleting.name}>
        {texts.deleteForever}
      </button>
    </div>
  </form>
{:else if accounts === null}
  {#if problem === null}
    <p>{texts.loading}</p>
  {/if}
{:else}
  <input type="search" bind:value={query} aria-label={texts.searchAccount} placeholder={texts.searchAccount} />
  {#if found.length === 0}
    <p class="muted">{texts.noAccountFound}</p>
  {/if}
  <Paged items={found} layout={ACCOUNT_LIST} label={texts.accountPages} bind:page>
    {#snippet children(visible)}
      <ul class="cards" aria-label={texts.adminTabs.accounts}>
        {#each visible as account (account.id)}
          <li>
            <h3>
              {account.name}
              {#if account.isAdmin}
                <span class="state">{texts.administrator}</span>
              {/if}
            </h3>
            {#if account.devices.length === 0}
              <p class="muted">{texts.noDevice}</p>
            {:else}
              <ul aria-label={texts.devices}>
                {#each account.devices as device (device.id)}
                  <li class="row">
                    <span>{texts.deviceAdded(device.name, device.createdAt)}</span>
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={texts.revokeDevice(device.name)}
                      onclick={() => revoke(device.id, device.name)}
                    >
                      {texts.revoke}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if account.isAdmin && admins === 1}
              <p class="muted">{texts.lastAdministrator}</p>
            {:else}
              <div class="actions">
                <button type="button" disabled={busy} onclick={() => toggleAdmin(account)}>
                  {account.isAdmin ? texts.removeAdmin : texts.makeAdmin}
                </button>
                {#if account.id === accountId}
                  <button type="button" class="danger" disabled={busy} onclick={() => startDeletion(account)}>
                    {texts.deleteOwnAccount}
                  </button>
                {:else if !account.isAdmin}
                  <button type="button" class="danger" disabled={busy} onclick={() => startDeletion(account)}>
                    {texts.deleteAccount}
                  </button>
                {/if}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/snippet}
  </Paged>
  <form onsubmit={add} novalidate>
    <h3>{texts.addMember}</h3>
    <label>
      {texts.memberName}
      <input bind:value={memberName} />
    </label>
    <div class="actions">
      <button type="submit" class="primary" disabled={busy}>{busy ? texts.creatingKey : texts.createKey}</button>
    </div>
  </form>
{/if}

<style>
  input[type="search"] {
    margin-bottom: var(--space-m);
  }

  .cards {
    display: grid;
    gap: var(--space-m);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  /* Deux cartes par ligne, comme le prévoit ACCOUNT_LIST. */
  @media (min-width: 48rem) {
    .cards {
      grid-template-columns: 1fr 1fr;
    }
  }

  .cards > li {
    padding: var(--space-m);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  h3 {
    margin-top: 0;
    overflow-wrap: anywhere;
  }

  .state {
    margin-left: var(--space-s);
    color: var(--text-muted);
    font-size: 0.875rem;
    font-weight: normal;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-s);
    margin-bottom: var(--space-s);
  }

  ul ul {
    padding-left: 0;
    list-style: none;
  }

  .muted {
    color: var(--text-muted);
  }

  .key code {
    overflow-wrap: anywhere;
  }
</style>
