<script lang="ts">
  import { deleteVault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";

  interface Props {
    name: string;
    onDeleted: () => void;
    onClose: () => void;
  }

  const { name, onDeleted, onClose }: Props = $props();

  let confirmation = $state("");
  let problem = $state<string | null>(null);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    try {
      await deleteVault(name, confirmation);
      onDeleted();
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<h3>{texts.deleteVault}</h3>
<form onsubmit={submit}>
  <p>{texts.deleteVaultExplanation(name)}</p>
  <label>
    {texts.vaultName}
    <input bind:value={confirmation} autocomplete="off" spellcheck="false" />
  </label>
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.cancel}</button>
    <button type="submit" class="danger" disabled={confirmation.trim() !== name}>{texts.deleteForever}</button>
  </div>
</form>
