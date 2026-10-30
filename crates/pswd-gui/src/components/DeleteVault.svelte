<script lang="ts">
  import { deleteVault, deleteVaultEverywhere } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";

  interface Props {
    name: string;
    /** Coffre privé publié sur le serveur de l'appareil : il peut aussi y être supprimé. */
    onServer?: boolean;
    onDeleted: () => void;
    onClose: () => void;
  }

  const { name, onServer = false, onDeleted, onClose }: Props = $props();

  let confirmation = $state("");
  let everywhere = $state(false);
  let problem = $state<string | null>(null);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    try {
      await (everywhere ? deleteVaultEverywhere : deleteVault)(name, confirmation);
      onDeleted();
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<h3>{texts.deleteVault}</h3>
<form onsubmit={submit}>
  <p>{texts.deleteVaultExplanation(name)}</p>
  {#if onServer}
    <fieldset>
      <legend>{texts.deleteWhere}</legend>
      <label class="check">
        <input type="radio" name="where" value={false} bind:group={everywhere} />
        {texts.deleteLocalOnly}
      </label>
      <label class="check">
        <input type="radio" name="where" value={true} bind:group={everywhere} />
        {texts.deleteEverywhere}
      </label>
    </fieldset>
    {#if everywhere}
      <p class="muted">{texts.deleteEverywhereExplanation}</p>
    {/if}
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
    <button type="submit" class="danger" disabled={confirmation.trim() !== name}>{texts.deleteForever}</button>
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
