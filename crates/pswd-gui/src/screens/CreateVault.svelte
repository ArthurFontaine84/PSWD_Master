<script lang="ts">
  import { createVault, type CreatedVault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { newVaultProblem } from "../lib/validation";

  interface Props {
    onCreated: (created: CreatedVault) => void;
    onCancel: () => void;
  }

  const { onCreated, onCancel }: Props = $props();

  let name = $state("");
  let password = $state("");
  let confirmation = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    problem = newVaultProblem(name, password, confirmation);
    if (problem !== null) {
      return;
    }
    const typed = password;
    // Le mot de passe ne reste pas dans les champs une fois envoyé.
    password = "";
    confirmation = "";
    busy = true;
    try {
      onCreated(await createVault(name, typed));
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-labelledby="create-title">
  <h2 id="create-title">{texts.newVault}</h2>
  <form onsubmit={submit} novalidate>
    <label>
      {texts.vaultName}
      <input bind:value={name} {...verbatim} />
      <small>{texts.vaultNameHint}</small>
    </label>
    <label>
      {texts.masterPassword}
      <input type="password" bind:value={password} autocomplete="new-password" />
    </label>
    <label>
      {texts.confirmation}
      <input type="password" bind:value={confirmation} autocomplete="new-password" />
    </label>
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" onclick={onCancel}>{texts.cancel}</button>
      <button type="submit" class="primary" disabled={busy}>
        {busy ? texts.creating : texts.create}
      </button>
    </div>
  </form>
</section>
