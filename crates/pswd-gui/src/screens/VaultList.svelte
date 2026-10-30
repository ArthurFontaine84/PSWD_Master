<script lang="ts">
  import { fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import {
    acceptInvitation,
    deleteInvitation,
    listInvitations,
    listVaults,
    serverStatus,
    syncServer,
    type Invitation,
    type VaultStatus,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { motion } from "../lib/motion";
  import { exchangeOutcome } from "../lib/sync";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    notice?: string;
    onOpen: (name: string) => void;
    onBackup: () => void;
    onRestore: () => void;
  }

  const { notice, onOpen, onBackup, onRestore }: Props = $props();
  let vaults = $state(listVaults());
  /** Sans coffre lisible, rien à sauvegarder : l'erreur s'affiche sous la liste. */
  const canBackUp = $derived(
    vaults.then(
      (names) => names.length > 0,
      () => false,
    ),
  );
  let connected = $state(false);
  /** Type de chaque coffre, et son état vis-à-vis du serveur, affiché seulement si l'appareil est connecté. */
  let states = $state(new Map<string, VaultStatus>());
  let invitations = $state<Invitation[]>([]);
  /** Une invitation traitée à la fois : son identifiant. */
  let answering = $state<string | null>(null);
  let syncing = $state(false);
  let problems = $state<string[]>([]);

  // Sans état lisible, la liste reste celle d'un appareil non connecté.
  async function loadStatus() {
    try {
      const status = await serverStatus();
      connected = status.address !== null;
      states = new Map(status.vaults.map((vault) => [vault.name, vault]));
    } catch {
      return;
    }
    if (connected) {
      await loadInvitations();
    }
  }

  /** Un serveur injoignable n'empêche pas d'ouvrir les coffres : son erreur est signalée. */
  async function loadInvitations() {
    try {
      invitations = await listInvitations();
    } catch (error) {
      problems = [...problems, errorMessage(error)];
    }
  }

  void loadStatus();

  async function accept(invitation: Invitation) {
    answering = invitation.id;
    problems = [];
    try {
      const outcome = exchangeOutcome(await acceptInvitation(invitation.id));
      showToast([...outcome.done, texts.invitationAccepted(invitation.invitedBy)].join(" "));
      problems = outcome.problems;
      vaults = listVaults();
      await loadStatus();
    } catch (error) {
      problems = [errorMessage(error)];
    } finally {
      answering = null;
    }
  }

  async function decline(invitation: Invitation) {
    answering = invitation.id;
    problems = [];
    try {
      await deleteInvitation(invitation.id);
      showToast(texts.invitationDeclined(invitation.vaultName));
      await loadInvitations();
    } catch (error) {
      problems = [errorMessage(error)];
    } finally {
      answering = null;
    }
  }

  /** Télécharge les nouveaux coffres du compte et publie ceux de l'appareil. */
  async function synchronize() {
    syncing = true;
    problems = [];
    try {
      const outcome = exchangeOutcome(await syncServer());
      if (outcome.done.length > 0) {
        showToast(outcome.done.join(" "));
      }
      problems = outcome.problems;
      vaults = listVaults();
      await loadStatus();
    } catch (error) {
      problems = [errorMessage(error)];
    } finally {
      syncing = false;
    }
  }
</script>

<section aria-labelledby="vaults-title">
  <div class="toolbar">
    {#await canBackUp then shown}
      {#if shown}
        <button type="button" onclick={onBackup}><Icon name="download" />{texts.backup}</button>
      {/if}
    {/await}
    <button type="button" onclick={onRestore}><Icon name="restore" />{texts.restore}</button>
    {#if connected}
      <button type="button" disabled={syncing} onclick={synchronize}>
        <Icon name="sync" />{syncing ? texts.synchronizing : texts.synchronize}
      </button>
    {/if}
  </div>
  {#if notice}
    <p role="status" class="notice">{notice}</p>
  {/if}
  {#each problems as message (message)}
    <p role="alert">{message}</p>
  {/each}
  {#if invitations.length > 0}
    <h2>{texts.invitations}</h2>
    <ul>
      {#each invitations as invitation (invitation.id)}
        <li class="invitation">
          <p>{texts.invitation(invitation.vaultName, invitation.invitedBy)}</p>
          <div class="actions">
            <button type="button" disabled={answering !== null} onclick={() => decline(invitation)}>
              {texts.decline}
            </button>
            <button type="button" class="primary" disabled={answering !== null} onclick={() => accept(invitation)}>
              {texts.accept}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  <h2 id="vaults-title">{texts.vaults}</h2>
  {#await vaults}
    <p class="muted">{texts.loading}</p>
  {:then names}
    {#if names.length === 0}
      <p class="muted">{texts.noVault}</p>
    {:else}
      <ul>
        {#each names as name, index (name)}
          {@const status = states.get(name)}
          <li in:fly={{ y: 12, duration: motion(300), delay: motion(Math.min(index, 8) * 40) }}>
            <button type="button" class="vault" onclick={() => onOpen(name)}>
              <span class="badge"><Icon name="lock" /></span>
              <span class="name" title={name}>{name}</span>
              {#if status?.shared}
                <span class="state">{texts.shared}</span>
              {/if}
              {#if connected && status !== undefined && status.state !== "published"}
                <span class="state">{texts.vaultStates[status.state]}</span>
              {/if}
              <Icon name="chevron" />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:catch error}
    <p role="alert">{errorMessage(error)}</p>
  {/await}
</section>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-s);
  }

  h2 {
    margin: var(--space-l) 0 var(--space-m);
  }

  .muted {
    color: var(--text-muted);
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: var(--space-s);
  }

  .invitation {
    padding: var(--space-m);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    -webkit-backdrop-filter: var(--blur);
    backdrop-filter: var(--blur);
  }

  .invitation p {
    margin: 0 0 var(--space-s);
  }

  /* Sans cela, un nom long élargirait sa colonne au lieu d'être coupé. */
  li {
    min-width: 0;
  }

  .vault {
    width: 100%;
    justify-content: flex-start;
    gap: var(--space-m);
    padding: var(--space-m);
    border-radius: var(--radius-l);
    box-shadow: var(--shadow);
    color: var(--text-muted);
    -webkit-backdrop-filter: var(--blur);
    backdrop-filter: var(--blur);
  }

  @media (hover: hover) {
    .vault:hover {
      transform: translateY(-2px);
      box-shadow: var(--shadow-raised);
    }
  }

  .badge {
    flex: none;
    display: inline-flex;
    padding: 0.5rem;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    color: var(--text);
    /* Une seule ligne, coupée par « … » : toutes les cartes gardent la même taille. */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .state {
    flex: none;
    padding: 0.125rem 0.5rem;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.75rem;
    font-weight: 600;
  }

  /* Écran large : les coffres sur plusieurs colonnes. */
  @media (min-width: 48rem) {
    ul {
      grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    }
  }

  /* Très grand écran : des cartes plus hautes et plus espacées. */
  @media (min-width: 100rem) and (min-height: 56rem) {
    ul {
      gap: var(--space-m);
    }

    .vault {
      padding: var(--space-l) var(--space-m);
      font-size: 1.125rem;
    }
  }
</style>
