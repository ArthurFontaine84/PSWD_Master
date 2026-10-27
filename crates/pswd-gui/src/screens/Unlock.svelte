<script lang="ts">
  import { recoverVault, unlockVault, type VaultInfo } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { newPasswordProblem } from "../lib/validation";

  interface Props {
    name: string;
    onUnlocked: (vault: VaultInfo) => void;
    onCancel: () => void;
  }

  const { name, onUnlocked, onCancel }: Props = $props();

  /** `true` : mot de passe oublié, la clé de récupération en fixe un nouveau. */
  let recovering = $state(false);
  let password = $state("");
  let recoveryKey = $state("");
  let confirmation = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);

  /** Les secrets ne restent pas dans les champs une fois envoyés. */
  function clear() {
    password = "";
    recoveryKey = "";
    confirmation = "";
  }

  async function send(open: () => Promise<VaultInfo>) {
    busy = true;
    try {
      onUnlocked(await open());
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const typed = { password, recoveryKey, confirmation };
    if (!recovering) {
      clear();
      await send(() => unlockVault(name, typed.password));
      return;
    }
    problem = newPasswordProblem(typed.password, typed.confirmation);
    if (problem !== null) {
      return;
    }
    clear();
    await send(() => recoverVault(name, typed.recoveryKey, typed.password));
  }

  function startRecovery() {
    recovering = true;
    problem = null;
    clear();
  }
</script>

<section class="panel" aria-labelledby="unlock-title">
  <h2 id="unlock-title">{recovering ? texts.recoverTitle(name) : texts.unlockTitle(name)}</h2>
  <form onsubmit={submit} novalidate>
    {#if recovering}
      <label>
        {texts.recoveryKey}
        <input bind:value={recoveryKey} {...verbatim} />
      </label>
      <label>
        {texts.newPassword}
        <input type="password" bind:value={password} autocomplete="new-password" />
        <small>{texts.passwordRule}</small>
      </label>
      <label>
        {texts.confirmation}
        <input type="password" bind:value={confirmation} autocomplete="new-password" />
      </label>
    {:else}
      <label>
        {texts.masterPassword}
        <!-- svelte-ignore a11y_autofocus -->
        <input type="password" bind:value={password} autocomplete="current-password" autofocus />
      </label>
    {/if}
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    {#if !recovering}
      <button type="button" class="link" onclick={startRecovery}>{texts.forgotPassword}</button>
    {/if}
    <div class="actions">
      <button type="button" onclick={onCancel}>{texts.cancel}</button>
      <button type="submit" class="primary" disabled={busy}>
        {busy ? texts.unlocking : recovering ? texts.recover : texts.unlock}
      </button>
    </div>
  </form>
</section>

<style>
  .link {
    justify-self: start;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--accent);
    font-weight: 500;
  }

  @media (hover: hover) {
    .link:hover:not(:disabled) {
      background: none;
      text-decoration: underline;
    }
  }
</style>
