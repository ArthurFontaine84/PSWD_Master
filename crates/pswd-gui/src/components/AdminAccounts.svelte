<script lang="ts">
  import { addMember, adminAccounts, deleteAccount, revokeDevice, setAdmin, type AdminAccount } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    /** Adresse du serveur, à transmettre avec la clé d'un nouveau membre. */
    address: string;
  }

  const { address }: Props = $props();

  let accounts = $state<AdminAccount[] | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  let memberName = $state("");
  /** Clé d'un nouveau membre : montrée une fois, oubliée en la fermant. */
  let issued = $state<{ name: string; key: string } | null>(null);
  /** Compte dont la suppression attend que son nom soit retapé. */
  let deleting = $state<AdminAccount | null>(null);
  let confirmation = $state("");

  async function load() {
    try {
      accounts = await adminAccounts();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  void load();

  /** Une opération à la fois ; la liste est relue ensuite. */
  async function run(action: () => Promise<string | null>) {
    busy = true;
    problem = null;
    try {
      const message = await action();
      if (message !== null) {
        showToast(message);
      }
      await load();
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
    await run(async () => {
      await deleteAccount(account.id, account.name, confirmation);
      deleting = null;
      return texts.accountDeleted(account.name);
    });
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
  <h3>{texts.deleteAccount}</h3>
  <form onsubmit={remove}>
    <p>{texts.deleteAccountExplanation(deleting.name)}</p>
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
  <ul class="cards" aria-label={texts.adminTabs.accounts}>
    {#each accounts as account (account.id)}
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
                <button type="button" disabled={busy} onclick={() => revoke(device.id, device.name)}>
                  {texts.revoke(device.name)}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        <div class="actions">
          <button type="button" disabled={busy} onclick={() => toggleAdmin(account)}>
            {account.isAdmin ? texts.removeAdmin : texts.makeAdmin}
          </button>
          <button type="button" class="danger" disabled={busy} onclick={() => startDeletion(account)}>
            {texts.deleteAccount}
          </button>
        </div>
      </li>
    {/each}
  </ul>
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
  .cards {
    display: grid;
    gap: var(--space-m);
    padding: 0;
    list-style: none;
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
