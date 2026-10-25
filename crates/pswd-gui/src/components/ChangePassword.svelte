<script lang="ts">
  import { changePassword } from "../lib/api";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { newPasswordProblem } from "../lib/validation";

  interface Props {
    onClose: () => void;
    onExpired: () => void;
  }

  const { onClose, onExpired }: Props = $props();

  let current = $state("");
  let password = $state("");
  let confirmation = $state("");
  let problem = $state<string | null>(null);
  let done = $state(false);
  let busy = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    done = false;
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
      done = true;
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
  <label>
    {texts.currentPassword}
    <input type="password" bind:value={current} autocomplete="current-password" />
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
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  {#if done}
    <p role="status">{texts.masterPasswordChanged}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.cancel}</button>
    <button type="submit" class="primary" disabled={busy}>{texts.save}</button>
  </div>
</form>
