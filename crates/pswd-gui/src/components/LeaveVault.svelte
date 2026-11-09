<script lang="ts">
  import { onMount } from "svelte";
  import { deleteVaultEverywhere, leaveVault, vaultMembers } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    name: string;
    onLeft: () => void;
    /** Le dernier membre a supprimé le coffre ; `message` le dit. */
    onDeleted: (message: string) => void;
    onClose: () => void;
  }

  const { name, onLeft, onDeleted, onClose }: Props = $props();

  let problem = $state<string | null>(null);
  let busy = $state(false);
  /** Le compte est le seul membre : quitter devient supprimer ; `null` le temps de le savoir. */
  let lastMember = $state<boolean | null>(null);
  let confirmation = $state("");

  // Membres illisibles : « Quitter » reste proposé, le serveur dira ce qui ne va pas.
  onMount(() => {
    vaultMembers(name).then(
      ({ members }) => (lastMember = members.length === 1),
      () => (lastMember = false),
    );
  });

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

  async function deleteEverywhere(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    problem = null;
    try {
      await deleteVaultEverywhere(name, confirmation);
      onDeleted(texts.vaultDeleted(name));
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      busy = false;
    }
  }
</script>

{#if lastMember === null}
  <h3>{texts.leaveVault}</h3>
  <p>{texts.loading}</p>
{:else if lastMember}
  <h3>{texts.deleteVault}</h3>
  <form onsubmit={deleteEverywhere}>
    <p>{texts.lastMemberExplanation(name)}</p>
    <label>
      {texts.vaultName}
      <input bind:value={confirmation} {...verbatim} />
    </label>
    {#if problem}
      <p role="alert">{problem}</p>
    {/if}
    <div class="actions">
      <button type="button" onclick={onClose}>{texts.cancel}</button>
      <button type="submit" class="danger" disabled={busy || confirmation.trim() !== name}>
        {texts.deleteForever}
      </button>
    </div>
  </form>
{:else}
  <h3>{texts.leaveVault}</h3>
  <p>{texts.leaveExplanation(name)}</p>
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onClose}>{texts.cancel}</button>
    <button type="button" class="danger" disabled={busy} onclick={leave}>{texts.leave}</button>
  </div>
{/if}
