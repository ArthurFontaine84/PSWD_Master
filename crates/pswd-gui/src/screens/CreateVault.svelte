<script lang="ts">
  import PasswordInput from "../components/PasswordInput.svelte";
  import { createVault, type CreatedVault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { cleanVaultName, newVaultProblem } from "../lib/validation";

  interface Props {
    onCreated: (created: CreatedVault) => void;
    onCancel: () => void;
  }

  const { onCreated, onCancel }: Props = $props();

  let name = $state("");
  let password = $state("");
  let confirmation = $state("");
  let shared = $state(false);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const cleaned = cleanVaultName(name);
    problem = newVaultProblem(cleaned, password, confirmation);
    if (problem !== null) {
      return;
    }
    const typed = password;
    // Le mot de passe ne reste pas dans les champs une fois envoyé.
    password = "";
    confirmation = "";
    busy = true;
    try {
      onCreated(await createVault(cleaned, typed, shared));
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel" aria-labelledby="create-title">
  <h2 id="create-title">{texts.newVault}</h2>
  <form onsubmit={submit} novalidate>
    <label>
      {texts.vaultName}
      <input bind:value={name} {...verbatim} />
      <small>{texts.vaultNameHint}</small>
    </label>
    <PasswordInput
      bind:value={password}
      label={texts.masterPassword}
      hint={texts.passwordRule}
      autocomplete="new-password"
    />
    <PasswordInput bind:value={confirmation} label={texts.confirmation} autocomplete="new-password" />
    <label class="check">
      <input type="checkbox" bind:checked={shared} />
      {texts.sharedVault}
    </label>
    <small>{texts.sharedVaultHint}</small>
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
