<script lang="ts">
  import {
    accountStatus,
    addDeviceKey,
    connectServer,
    disconnectServer,
    serverStatus,
    syncServer,
    type AccountStatus,
    type ServerStatus,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { disconnectionOutcome, exchangeOutcome, type Outcome } from "../lib/sync";
  import { showToast } from "../lib/toast.svelte";
  import { connectionProblem } from "../lib/validation";

  interface Props {
    onClose: () => void;
  }

  const { onClose }: Props = $props();

  type Task = "connect" | "sync" | "key" | "disconnect";

  let status = $state<ServerStatus | null>(null);
  /** Compte de l'appareil ; inconnu tant que le serveur n'a pas répondu. */
  let account = $state<AccountStatus | null>(null);
  let problem = $state<string | null>(null);
  /** Échecs coffre par coffre du dernier échange ou de la déconnexion. */
  let problems = $state<string[]>([]);
  let running = $state<Task | null>(null);
  let address = $state("");
  let key = $state("");
  let deviceName = $state("");
  let accountName = $state("");
  /** Clé d'un nouvel appareil : montrée une fois, oubliée en quittant l'écran. */
  let deviceKey = $state<string | null>(null);
  let confirming = $state(false);
  let keepLocal = $state(true);

  async function load() {
    try {
      status = await serverStatus();
    } catch (error) {
      problem = errorMessage(error);
      return;
    }
    // Serveur injoignable : l'écran reste utilisable, sans le compte.
    account = status.address === null ? null : await accountStatus().catch(() => null);
  }

  void load();

  function report(outcome: Outcome) {
    if (outcome.done.length > 0) {
      showToast(outcome.done.join(" "));
    }
    problems = outcome.problems;
  }

  /** Une seule opération à la fois ; son erreur s'affiche dans l'écran. */
  async function run(task: Task, action: () => Promise<void>) {
    running = task;
    problem = null;
    problems = [];
    try {
      await action();
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      running = null;
    }
  }

  async function connect(event: SubmitEvent) {
    event.preventDefault();
    problem = connectionProblem(address, key, deviceName);
    if (problem !== null) {
      return;
    }
    const typed = key;
    key = "";
    await run("connect", async () => {
      await connectServer(address, typed, deviceName, accountName);
      await load();
      try {
        const outcome = exchangeOutcome(await syncServer());
        report({ done: [texts.connected, ...outcome.done], problems: outcome.problems });
      } catch (error) {
        showToast(texts.connected);
        problem = texts.exchangeIncomplete(errorMessage(error));
      }
    });
  }

  async function synchronize() {
    await run("sync", async () => {
      report(exchangeOutcome(await syncServer()));
      await load();
    });
  }

  async function newDeviceKey() {
    await run("key", async () => {
      deviceKey = await addDeviceKey();
    });
  }

  async function disconnect() {
    await run("disconnect", async () => {
      const outcome = disconnectionOutcome(await disconnectServer(keepLocal));
      confirming = false;
      report(outcome);
      await load();
    });
  }

  function startDisconnection() {
    keepLocal = true;
    problem = null;
    confirming = true;
  }
</script>

<section class="panel" aria-labelledby="server-title">
  <h2 id="server-title">{texts.serverTitle}</h2>
  {#each problems as message (message)}
    <p role="alert">{message}</p>
  {/each}
  {#if status === null}
    {#if problem}
      <p role="alert">{problem}</p>
      <div class="actions">
        <button type="button" onclick={onClose}>{texts.back}</button>
      </div>
    {:else}
      <p>{texts.loading}</p>
    {/if}
  {:else if status.address === null}
    <p class="explanation">{texts.serverExplanation}</p>
    <form onsubmit={connect} novalidate>
      <label>
        {texts.serverAddress}
        <input bind:value={address} {...verbatim} />
        <small>{texts.serverAddressHint}</small>
      </label>
      <label>
        {texts.connectionKey}
        <input bind:value={key} {...verbatim} />
        <small>{texts.connectionKeyHint}</small>
      </label>
      <label>
        {texts.deviceName}
        <input bind:value={deviceName} />
        <small>{texts.deviceNameHint}</small>
      </label>
      <label>
        {texts.accountName}
        <input bind:value={accountName} />
        <small>{texts.accountNameHint}</small>
      </label>
      {#if problem}
        <p role="alert">{problem}</p>
      {/if}
      <div class="actions">
        <button type="button" onclick={onClose}>{texts.back}</button>
        <button type="submit" class="primary" disabled={running !== null}>
          {running === "connect" ? texts.connecting : texts.connect}
        </button>
      </div>
    </form>
  {:else if deviceKey !== null}
    <h3>{texts.deviceKeyTitle}</h3>
    <p>{texts.deviceKeyExplanation(status.address)}</p>
    <p class="key"><code>{deviceKey}</code></p>
    <div class="actions">
      <button type="button" class="primary" onclick={() => (deviceKey = null)}>{texts.close}</button>
    </div>
  {:else if confirming}
    <h3>{texts.disconnectTitle}</h3>
    <p>{texts.disconnectExplanation}</p>
    <label class="check">
      <input type="checkbox" bind:checked={keepLocal} />
      {texts.keepLocalCopies}
    </label>
    {#if !keepLocal}
      <p class="explanation">{texts.removeExplanation}</p>
    {/if}
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" onclick={() => (confirming = false)}>{texts.cancel}</button>
      <button type="button" class="danger" disabled={running !== null} onclick={disconnect}>
        {running === "disconnect" ? texts.disconnecting : texts.disconnect}
      </button>
    </div>
  {:else}
    <p>{texts.connectedTo(status.address)}</p>
    {#if account !== null}
      <p>{texts.account(account.accountName, account.isAdmin)}</p>
    {/if}
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" class="primary" disabled={running !== null} onclick={synchronize}>
        {running === "sync" ? texts.synchronizing : texts.synchronize}
      </button>
      <button type="button" disabled={running !== null} onclick={newDeviceKey}>{texts.addDevice}</button>
    </div>
    <div class="actions">
      <button type="button" onclick={onClose}>{texts.back}</button>
      <button type="button" class="danger" disabled={running !== null} onclick={startDisconnection}>
        {texts.disconnect}
      </button>
    </div>
  {/if}
</section>

<style>
  .explanation {
    color: var(--text-muted);
  }
</style>
