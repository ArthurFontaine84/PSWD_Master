<script lang="ts">
  import { unlockVault, type VaultInfo } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";

  interface Props {
    name: string;
    onUnlocked: (vault: VaultInfo) => void;
    onCancel: () => void;
  }

  const { name, onUnlocked, onCancel }: Props = $props();

  let password = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const typed = password;
    // Le mot de passe ne reste pas dans le champ une fois envoyé.
    password = "";
    busy = true;
    try {
      onUnlocked(await unlockVault(name, typed));
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-labelledby="unlock-title">
  <h2 id="unlock-title">{texts.unlockTitle(name)}</h2>
  <form onsubmit={submit}>
    <label>
      {texts.masterPassword}
      <!-- svelte-ignore a11y_autofocus -->
      <input type="password" bind:value={password} autocomplete="current-password" autofocus />
    </label>
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" onclick={onCancel}>{texts.cancel}</button>
      <button type="submit" class="primary" disabled={busy}>
        {busy ? texts.unlocking : texts.unlock}
      </button>
    </div>
  </form>
</section>
