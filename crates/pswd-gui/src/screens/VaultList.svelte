<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "../components/Icon.svelte";
  import StateDot from "../components/StateDot.svelte";
  import {
    acceptInvitation,
    deleteInvitation,
    listInvitations,
    listVaults,
    refreshServer,
    serverStatus,
    syncServer,
    type Invitation,
    type ServerStatus,
    type VaultStatus,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { motion } from "../lib/motion";
  import { exchangeOutcome } from "../lib/sync";
  import { showToast } from "../lib/toast.svelte";
  import { vaultDot, type VaultDot } from "../lib/vaultDot";

  interface Props {
    notice?: string;
    /** `dot` : la pastille du coffre au moment de l'ouvrir, `null` sans pastille. */
    /** `shared` : coffre commun. */
    onOpen: (name: string, dot: VaultDot | null, shared: boolean) => void;
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
  let server = $state<Pick<ServerStatus, "address" | "trouble">>({ address: null, trouble: null });
  const connected = $derived(server.address !== null);
  /** Type de chaque coffre, et son état vis-à-vis du serveur, inconnu sans serveur joignable. */
  let states = $state(new Map<string, VaultStatus>());
  let invitations = $state<Invitation[]>([]);
  /** Une invitation traitée à la fois : son identifiant. */
  let answering = $state<string | null>(null);
  let syncing = $state(false);
  let problems = $state<string[]>([]);
  function show(status: ServerStatus) {
    server = { address: status.address, trouble: status.trouble };
    states = new Map(status.vaults.map((vault) => [vault.name, vault]));
  }

  // Sans état lisible, la liste reste celle d'un appareil non connecté.
  async function loadStatus() {
    try {
      show(await serverStatus());
    } catch {
      return;
    }
    if (connected) {
      await loadInvitations();
    }
  }

  /**
   * Un serveur injoignable n'empêche pas d'ouvrir les coffres : son erreur est signalée, sauf
   * pendant l'échange automatique, muet.
   */
  async function loadInvitations(quiet = false) {
    try {
      const fresh = await listInvitations();
      if (quiet) {
        announceInvitations(fresh);
      }
      invitations = fresh;
    } catch (error) {
      if (!quiet) {
        problems = [...problems, errorMessage(error)];
      }
    }
  }

  /** Échange automatique (SPEC §7) ; sans effet si un échange est déjà en cours. */
  async function refresh() {
    let status: ServerStatus | null;
    try {
      status = await refreshServer();
    } catch {
      return;
    }
    if (status === null) {
      return;
    }
    show(status);
    const renamed = status.renamed ?? [];
    if (renamed.length > 0) {
      showToast(renamed.map(({ name, newName }) => texts.gaveWay(name, newName)).join(" "));
    }
    await reloadNames(renamed.map(({ newName }) => newName));
    if (connected && status.trouble === null) {
      await loadInvitations(true);
    }
  }

  /**
   * Invitations reçues pendant l'échange automatique, signalées une fois : celles déjà là à
   * l'affichage de la liste sont visibles en haut, sans notification.
   */
  let knownInvitations: Set<string> | null = null;

  function announceInvitations(fresh: Invitation[]) {
    const known = knownInvitations;
    knownInvitations = new Set(fresh.map((invitation) => invitation.id));
    if (known === null) {
      return;
    }
    const arrived = fresh.filter((invitation) => !known.has(invitation.id));
    if (arrived.length > 0) {
      showToast(arrived.map(({ vaultName, invitedBy }) => texts.newInvitation(vaultName, invitedBy)).join(" "));
    }
  }

  /**
   * Les cartes ne sont redessinées que si les noms ont changé ; un nouveau coffre est signalé,
   * mais pas un coffre de l'appareil qui vient d'être renommé (`renamed` : ses nouveaux noms).
   */
  async function reloadNames(renamed: string[]) {
    const fresh = listVaults();
    const [before, after] = await Promise.all([vaults.catch(() => null), fresh.catch(() => null)]);
    const unchanged =
      before === null
        ? after === null
        : after !== null && before.length === after.length && before.every((name, i) => name === after[i]);
    if (unchanged) {
      return;
    }
    vaults = fresh;
    const arrived =
      before === null || after === null
        ? []
        : after.filter((name) => !before.includes(name) && !renamed.includes(name));
    if (arrived.length > 0) {
      showToast(arrived.map((name) => texts.vaultArrived(name)).join(" "));
    }
  }

  function refreshIfVisible() {
    if (document.visibilityState === "visible") {
      void refresh();
    }
  }

  onMount(() => {
    void refresh();
    const timer = setInterval(refreshIfVisible, 30_000);
    return () => clearInterval(timer);
  });

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

  /** Échange avec le serveur, sans mot de passe : téléchargement, publication, mise à jour. */
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

<svelte:window onfocus={refreshIfVisible} />
<svelte:document onvisibilitychange={refreshIfVisible} />

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
          {@const dot = vaultDot(server, status?.state)}
          <li in:fly={{ y: 12, duration: motion(300), delay: motion(Math.min(index, 8) * 40) }}>
            <div class="vault">
              <button type="button" class="open" onclick={() => onOpen(name, dot, status?.shared === true)}>
                <span class="badge"><Icon name="lock" /></span>
                <span class="name" title={name}>{name}</span>
                {#if status?.shared}
                  <span class="state">{texts.shared}</span>
                {/if}
              </button>
              {#if dot}
                {@const label = texts.vaultStates[dot]}
                <button type="button" class="dot" aria-label={label} title={label} onclick={() => showToast(label)}>
                  <StateDot {dot} />
                </button>
              {/if}
              <Icon name="chevron" />
            </div>
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

  /* La carte entière ouvre le coffre (bouton étendu par `::after`), sauf sa pastille. */
  .vault {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-m);
    padding: var(--space-m);
    border: 1px solid var(--border);
    border-radius: var(--radius-l);
    background: var(--surface);
    box-shadow: var(--shadow);
    color: var(--text-muted);
    -webkit-backdrop-filter: var(--blur);
    backdrop-filter: var(--blur);
    transition:
      background-color 0.2s var(--ease),
      border-color 0.2s var(--ease),
      box-shadow 0.2s var(--ease),
      transform 0.15s var(--ease);
  }

  .vault:has(.open:active) {
    transform: scale(0.98);
  }

  .vault:has(.open:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .open {
    flex: 1;
    min-width: 0;
    min-height: 0;
    justify-content: flex-start;
    gap: var(--space-m);
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: inherit;
    font-weight: inherit;
  }

  .open::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }

  .open:active:not(:disabled) {
    transform: none;
  }

  .open:focus-visible {
    outline: none;
  }

  /* Cible plus grande que la pastille, sans changer la taille de la carte. */
  .dot {
    position: relative;
    z-index: 1;
    flex: none;
    width: 2.25rem;
    height: 2.25rem;
    min-height: 0;
    margin: -0.75rem;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    box-shadow: none;
  }

  @media (hover: hover) {
    .vault:hover {
      transform: translateY(-2px);
      background: var(--surface-hover);
      border-color: var(--accent-soft);
      box-shadow: var(--shadow-raised);
    }

    .open:hover:not(:disabled) {
      background: none;
    }

    .dot:hover:not(:disabled) {
      background: var(--accent-soft);
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
