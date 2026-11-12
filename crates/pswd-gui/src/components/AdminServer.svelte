<script lang="ts">
  import { backupServerToFile, serverBackupFileName, setPublicAddress } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickServerBackupDestination } from "../lib/files";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { showToast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    publicAddress: string | null;
  }

  const { publicAddress }: Props = $props();

  // Valeur de départ du champ : l'adresse connue à l'ouverture du panneau.
  // svelte-ignore state_referenced_locally
  let address = $state(publicAddress ?? "");
  let problem = $state<string | null>(null);
  let running = $state<"address" | "backup" | null>(null);

  async function run(task: "address" | "backup", action: () => Promise<void>) {
    running = task;
    problem = null;
    try {
      await action();
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      running = null;
    }
  }

  async function saveAddress(event: SubmitEvent) {
    event.preventDefault();
    await run("address", async () => {
      await setPublicAddress(address);
      showToast(texts.publicAddressSaved);
    });
  }

  async function backup() {
    await run("backup", async () => {
      const path = await pickServerBackupDestination(texts.serverBackupFileTitle, await serverBackupFileName());
      if (path !== null) {
        await backupServerToFile(path);
        showToast(texts.serverBackedUp);
      }
    });
  }
</script>

{#if problem}
  <p role="alert">{problem}</p>
{/if}
<form onsubmit={saveAddress} novalidate>
  <label>
    {texts.publicAddress}
    <input bind:value={address} {...verbatim} />
    <small>{texts.publicAddressHint}</small>
  </label>
  <div class="actions">
    <button type="submit" class="primary" disabled={running !== null}>
      {running === "address" ? texts.saving : texts.save}
    </button>
  </div>
</form>
<div class="backup">
  <h3>{texts.serverBackup}</h3>
  <button
    type="button"
    class="icon-button"
    aria-label={texts.serverBackup}
    title={running === "backup" ? texts.backingUp : texts.serverBackup}
    aria-busy={running === "backup"}
    disabled={running !== null}
    onclick={backup}
  >
    <Icon name="save" />
  </button>
</div>
<p class="muted">{texts.serverBackupExplanation}</p>

<style>
  .backup {
    display: flex;
    align-items: center;
    gap: var(--space-m);
    margin-top: 1rem;
  }

  .muted {
    color: var(--text-muted);
  }
</style>
