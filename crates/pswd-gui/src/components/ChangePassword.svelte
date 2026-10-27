<script lang="ts">
  import { changePassword } from "../lib/api";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { showToast } from "../lib/toast.svelte";
  import { newPasswordProblem } from "../lib/validation";
  import PasswordInput from "./PasswordInput.svelte";

  interface Props {
    onClose: () => void;
    onExpired: () => void;
  }

  const { onClose, onExpired }: Props = $props();

  let current = $state("");
  let password = $state("");
  let confirmation = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    problem = newPasswordProblem(password, confirmation);
    if (problem !== null) {
      return;
    }
    const typed = { current, password };
    // Les mots de passe ne restent pas dans les champs une fois envoyés.
    current = "";
    password = "";
    confirmation = "";
    busy = true;
    try {
      await changePassword(typed.current, typed.password);
      showToast(texts.masterPasswordChanged);
      onClose();
    } catch (error) {
      if (isVaultLocked(error)) {
        onExpired();
      } else {
        problem = errorMessage(error);
      }
    } finally {
      busy = false;
    }
  }
</script>

<h3>{texts.changePassword}</h3>
<form onsubmit={submit} novalidate>
  <PasswordInput bind:value={current} label={texts.currentPassword} autocomplete="current-password" />
  <PasswordInput bind:value={password} label={texts.newPassword} hint={texts.passwordRule} autocomplete="new-password" />
  <PasswordInput bind:value={confirmation} label={texts.confirmation} autocomplete="new-password" />
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.cancel}</button>
    <button type="submit" class="primary" disabled={busy}>{texts.save}</button>
  </div>
</form>
