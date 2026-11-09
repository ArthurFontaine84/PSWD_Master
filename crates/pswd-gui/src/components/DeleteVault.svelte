<script lang="ts">
  import { deleteVault, deleteVaultEverywhere, forgetVault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";

  interface Props {
    name: string;
    /** Coffre privé publié sur le serveur de l'appareil : supprimé partout, ou retiré de l'appareil. */
    onServer?: boolean;
    /** `message` dit ce qui a été fait. */
    onDeleted: (message: string) => void;
    onClose: () => void;
  }

  const { name, onServer = false, onDeleted, onClose }: Props = $props();

  let confirmation = $state("");
  let choice = $state<"everywhere" | "device">("everywhere");
  let problem = $state<string | null>(null);
  const forgetting = $derived(onServer && choice === "device");

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    try {
      if (!onServer) {
        await deleteVault(name, confirmation);
      } else if (forgetting) {
        await forgetVault(name, confirmation);
      } else {
        await deleteVaultEverywhere(name, confirmation);
      }
      onDeleted(forgetting ? texts.removedFromDevice(name) : texts.vaultDeleted(name));
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<h3>{texts.deleteVault}</h3>
<form onsubmit={submit}>
  {#if onServer}
    <fieldset>
      <legend>{texts.deleteWhere}</legend>
      <label class="check">
        <input type="radio" name="where" value="everywhere" bind:group={choice} />
        {texts.deleteEverywhere}
      </label>
      <label class="check">
        <input type="radio" name="where" value="device" bind:group={choice} />
        {texts.forgetVault}
      </label>
    </fieldset>
    <p class="muted">{forgetting ? texts.forgetVaultExplanation : texts.deleteEverywhereExplanation}</p>
    <p>{texts.confirmVaultName}</p>
  {:else}
    <p>{texts.deleteVaultExplanation(name)}</p>
  {/if}
  <label>
    {texts.vaultName}
    <input bind:value={confirmation} {...verbatim} />
  </label>
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.cancel}</button>
    <button type="submit" class="danger" disabled={confirmation.trim() !== name}>
      {forgetting ? texts.forget : texts.deleteForever}
    </button>
  </div>
</form>

<style>
  fieldset {
    margin: 0 0 var(--space-m);
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: var(--space-s);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
