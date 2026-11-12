<script lang="ts">
  import { onMount } from "svelte";
  import {
    deleteInvitation,
    inviteMember,
    listAccounts,
    vaultMembers,
    type Account,
    type Members,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { EXCHANGE_INTERVAL } from "../lib/sync";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    name: string;
    onClose: () => void;
  }

  const { name, onClose }: Props = $props();

  let members = $state<Members | null>(null);
  let accounts = $state<Account[]>([]);
  let chosen = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);
  /** Comptes ni membres ni déjà invités. */
  const invitable = $derived(
    accounts.filter(
      ({ id }) =>
        members !== null &&
        !members.members.some((member) => member.id === id) &&
        !members.invitations.some((invitation) => invitation.account.id === id),
    ),
  );

  /** `quiet` : relecture automatique, dont l'échec n'est pas affiché. */
  async function load(quiet = false) {
    try {
      [members, accounts] = await Promise.all([vaultMembers(name), listAccounts()]);
      if (!invitable.some(({ id }) => id === chosen)) {
        chosen = invitable[0]?.id ?? "";
      }
    } catch (error) {
      if (!quiet) {
        problem = errorMessage(error);
      }
    }
  }

  void load();

  // Une invitation acceptée ailleurs fait passer le compte dans les membres, comme l'échange
  // automatique (SPEC §7).
  onMount(() => {
    const timer = setInterval(() => {
      if (document.visibilityState === "visible" && !busy) {
        void load(true);
      }
    }, EXCHANGE_INTERVAL);
    return () => clearInterval(timer);
  });

  /** Une opération à la fois ; la liste est relue ensuite. */
  async function run(action: () => Promise<string>) {
    busy = true;
    problem = null;
    try {
      const message = await action();
      showToast(message);
      await load();
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function invite(event: SubmitEvent) {
    event.preventDefault();
    const account = invitable.find(({ id }) => id === chosen);
    if (account === undefined) {
      return;
    }
    await run(async () => {
      await inviteMember(name, account.id);
      return texts.invited;
    });
  }

  async function cancel(id: string) {
    await run(async () => {
      await deleteInvitation(id);
      return texts.invitationCancelled;
    });
  }
</script>

<h3>{texts.membersTitle}</h3>
{#if problem}
  <p role="alert">{problem}</p>
{/if}
{#if members === null}
  {#if problem === null}
    <p>{texts.loading}</p>
  {/if}
{:else}
  <ul aria-label={texts.members}>
    {#each members.members as member (member.id)}
      <li>{member.name}</li>
    {/each}
  </ul>
  {#if members.invitations.length > 0}
    <h4>{texts.pendingInvitations}</h4>
    <ul aria-label={texts.pendingInvitations}>
      {#each members.invitations as invitation (invitation.id)}
        <li class="pending">
          <span>{invitation.account.name}</span>
          <button
            type="button"
            disabled={busy}
            onclick={() => cancel(invitation.id)}
          >
            {texts.cancelInvitation(invitation.account.name)}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if invitable.length === 0}
    <p class="muted">{texts.noOneToInvite}</p>
  {:else}
    <form onsubmit={invite}>
      <label>
        {texts.inviteAccount}
        <select bind:value={chosen}>
          {#each invitable as account (account.id)}
            <option value={account.id}>{account.name}</option>
          {/each}
        </select>
      </label>
      <div class="actions">
        <button type="submit" class="primary" disabled={busy}>{busy ? texts.inviting : texts.invite}</button>
      </div>
    </form>
  {/if}
{/if}
<div class="actions">
  <button type="button" onclick={onClose}>{texts.close}</button>
</div>

<style>
  ul {
    padding-left: var(--space-l);
  }

  .pending {
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
