<script lang="ts">
  import { onMount } from "svelte";
  import {
    addMember,
    adminAccounts,
    cancelKey,
    deleteAccount,
    pendingKeys,
    revokeDevice,
    setAdmin,
    type AdminAccount,
    type PendingKey,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { ACCOUNT_LIST } from "../lib/layout";
  import { everyExchange } from "../lib/poll";
  import { byName } from "../lib/search";
  import { showToast } from "../lib/toast.svelte";
  import Confirm from "./Confirm.svelte";
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

  /** Clé d'un nouveau membre : son compte n'existe qu'une fois la clé utilisée. */
  type MemberKey = PendingKey & { name: string };
  /** Carte de la liste : un compte, ou un membre en attente de sa première connexion. */
  type Item = { id: string; name: string } & ({ account: AdminAccount } | { key: MemberKey });
  /** Révocation d'un appareil ou annulation d'une clé, en attente de confirmation. */
  type Asking = { device: { id: string; name: string }; account: string } | { key: MemberKey };

  let accounts = $state<AdminAccount[] | null>(null);
  let items = $state<Item[]>([]);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  /** Formulaire d'ajout d'un membre ouvert. */
  let adding = $state(false);
  let memberName = $state("");
  /** Clé d'un nouveau membre : montrée une fois, oubliée en la fermant. */
  let issued = $state<{ name: string; key: string } | null>(null);
  /** Compte dont la suppression attend que son nom soit retapé. */
  let deleting = $state<AdminAccount | null>(null);
  let confirmation = $state("");
  let asking = $state<Asking | null>(null);
  const admins = $derived(accounts?.filter(({ isAdmin }) => isAdmin).length ?? 0);
  let query = $state("");
  let page = $state(0);
  const found = $derived(byName(items, query));

  // Une nouvelle recherche repart de la première page.
  $effect(() => {
    void query;
    page = 0;
  });

  $effect(() => {
    onConfirming(deleting !== null || asking !== null);
    return () => onConfirming(false);
  });

  /** `quiet` : relecture automatique, dont l'échec n'est pas affiché. */
  async function load(quiet = false) {
    try {
      const [list, keys] = await Promise.all([adminAccounts(), pendingKeys()]);
      const members = keys.filter((key): key is MemberKey => key.purpose === "member" && key.name !== null);
      items = [
        ...list.map((account) => ({ id: account.id, name: account.name, account })),
        ...members.map((key) => ({ id: key.id, name: key.name, key })),
      ];
      accounts = list;
    } catch (error) {
      if (!quiet) {
        problem = errorMessage(error);
      }
    }
  }

  void load();

  // Comptes et clés changent aussi depuis les autres appareils : la liste suit l'échange
  // automatique, sauf pendant une opération ou une confirmation.
  onMount(() =>
    everyExchange(() => {
      if (!busy && issued === null && deleting === null && asking === null) {
        void load(true);
      }
    }),
  );

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
    if (items.some((item) => item.name === name)) {
      problem = texts.validation.memberNameTaken;
      return;
    }
    await run(async () => {
      issued = { name, key: await addMember(name) };
      closeForm();
      return null;
    });
  }

  function openForm() {
    problem = null;
    adding = true;
  }

  function closeForm() {
    problem = null;
    adding = false;
    memberName = "";
  }

  async function toggleAdmin(account: AdminAccount) {
    await run(async () => {
      await setAdmin(account.id, !account.isAdmin);
      return account.isAdmin ? texts.roleRemoved(account.name) : texts.roleGiven(account.name);
    });
  }

  function ask(what: Asking) {
    problem = null;
    asking = what;
  }

  async function confirmAsked(what: Asking) {
    await run(async () => {
      if ("key" in what) {
        await cancelKey(what.key.id);
        asking = null;
        return texts.keyCancelled;
      }
      await revokeDevice(what.device.id);
      asking = null;
      return texts.deviceRevoked(what.device.name);
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
{:else if asking !== null}
  {@const what = asking}
  {#if "key" in what}
    <Confirm
      title={texts.cancelKey}
      message={texts.cancelKeyExplanation(texts.keyPurpose("member", what.key.name))}
      action={texts.cancelKey}
      dismiss={texts.keepKey}
      {busy}
      onConfirm={() => confirmAsked(what)}
      onCancel={() => (asking = null)}
    />
  {:else}
    <Confirm
      title={texts.revokeDeviceTitle}
      message={texts.revokeDeviceExplanation(what.device.name, what.account)}
      action={texts.revoke}
      {busy}
      onConfirm={() => confirmAsked(what)}
      onCancel={() => (asking = null)}
    />
  {/if}
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
        {#each visible as item (item.id)}
          <li>
            {#if "key" in item}
              <h3>
                {item.name}
                <span class="state">{texts.pendingMember}</span>
              </h3>
              <p class="muted">{texts.keyNotUsed}</p>
              <div class="actions">
                <button
                  type="button"
                  disabled={busy}
                  aria-label={texts.cancelKeyOf(item.name)}
                  onclick={() => ask({ key: item.key })}
                >
                  {texts.cancelKey}
                </button>
              </div>
            {:else}
              {@render accountCard(item.account)}
            {/if}
          </li>
        {/each}
      </ul>
    {/snippet}
  </Paged>
  {#if adding}
    <form onsubmit={add} novalidate class="new-member">
      <label>
        {texts.memberName}
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={memberName} autofocus />
      </label>
      <div class="actions">
        <button type="button" onclick={closeForm}>{texts.cancel}</button>
        <button type="submit" class="primary" disabled={busy}>{busy ? texts.creatingKey : texts.save}</button>
      </div>
    </form>
  {:else}
    <div class="actions new-member">
      <button type="button" class="primary" onclick={openForm}>{texts.addMember}</button>
    </div>
  {/if}
{/if}

{#snippet accountCard(account: AdminAccount)}
  {@const lastAdmin = account.isAdmin && admins === 1}
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
          <!-- Le dernier administrateur garde son dernier appareil, comme son rôle. -->
          {#if !lastAdmin || account.devices.length > 1}
            <button
              type="button"
              disabled={busy}
              aria-label={texts.revokeDevice(device.name)}
              onclick={() => ask({ device, account: account.name })}
            >
              {texts.revoke}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  {#if lastAdmin}
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
{/snippet}

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

  .new-member {
    margin-top: var(--space-m);
  }
</style>
