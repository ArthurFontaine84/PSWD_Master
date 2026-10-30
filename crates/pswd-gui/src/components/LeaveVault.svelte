<script lang="ts">
  import { leaveVault } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    name: string;
    onLeft: () => void;
    onClose: () => void;
  }

  const { name, onLeft, onClose }: Props = $props();

  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function leave() {
    busy = true;
    problem = null;
    try {
      await leaveVault(name);
      showToast(texts.vaultLeft(name));
      onLeft();
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

<h3>{texts.leaveVault}</h3>
<p>{texts.leaveExplanation(name)}</p>
{#if problem}
  <p role="alert">{problem}</p>
{/if}
<div class="actions">
  <button type="button" onclick={onClose}>{texts.cancel}</button>
  <button type="button" class="danger" disabled={busy} onclick={leave}>{texts.leave}</button>
</div>
