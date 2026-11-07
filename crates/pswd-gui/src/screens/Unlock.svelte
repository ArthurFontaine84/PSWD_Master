<script lang="ts">
  import PasswordInput from "../components/PasswordInput.svelte";
  import StateDot from "../components/StateDot.svelte";
  import { recoverVault, unlockVault, type VaultInfo } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { newPasswordProblem } from "../lib/validation";
  import type { VaultDot } from "../lib/vaultDot";

  interface Props {
    name: string;
    /** Pastille du coffre dans la liste, affichée avec son texte ; `null` sans serveur enregistré. */
    dot?: VaultDot | null;
    onUnlocked: (vault: VaultInfo) => void;
    onCancel: () => void;
  }

  const { name, dot = null, onUnlocked, onCancel }: Props = $props();

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
  {#if dot}
    <p class="server-state"><StateDot {dot} />{texts.vaultStates[dot]}</p>
  {/if}
  {#if dot === "removed"}
    <p role="status" class="notice">{texts.removedNotice}</p>
  {/if}
  <form onsubmit={submit} novalidate>
    {#if recovering}
      <label>
        {texts.recoveryKey}
        <input bind:value={recoveryKey} {...verbatim} />
      </label>
      <PasswordInput
        bind:value={password}
        label={texts.newPassword}
        hint={texts.passwordRule}
        autocomplete="new-password"
      />
      <PasswordInput bind:value={confirmation} label={texts.confirmation} autocomplete="new-password" />
    {:else}
      <PasswordInput bind:value={password} label={texts.masterPassword} autocomplete="current-password" autofocus />
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
  .server-state {
    display: flex;
    align-items: center;
    gap: var(--space-s);
    margin: 0 0 var(--space-m);
    color: var(--text-muted);
  }

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
