<script lang="ts">
  import { backupServerToFile, serverBackupFileName } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { pickServerBackupDestination } from "../lib/files";
  import { texts } from "../lib/i18n";
    import { showToast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    publicAddress: string | null;
  }

  const { publicAddress }: Props = $props();

  let problem = $state<string | null>(null);
  let running = $state(false);

  async function backup() {
    running = true;
    problem = null;
    try {
      const path = await pickServerBackupDestination(texts.serverBackupFileTitle, await serverBackupFileName());
      if (path !== null) {
        await backupServerToFile(path);
        showToast(texts.serverBackedUp);
      }
    } catch (error) {
      problem = errorMessage(error);
    } finally {
      running = false;
    }
  }
</script>

{#if problem}
  <p role="alert">{problem}</p>
{/if}
<!-- EXTENSION(acces-exterieur): rendre le champ modifiable (`setPublicAddress`) quand l'accès depuis l'extérieur sera prêt. -->
<label>
  {texts.publicAddress}
  <input value={publicAddress ?? ""} disabled />
  <small>{texts.publicAddressComingSoon}</small>
</label>
<div class="backup">
  <h3>{texts.serverBackup}</h3>
  <button
    type="button"
    class="icon-button"
    aria-label={texts.serverBackup}
    title={running ? texts.backingUp : texts.serverBackup}
    aria-busy={running}
    disabled={running}
    onclick={backup}
  >
    <Icon name="save" />
  </button>
</div>
<p class="muted">{texts.serverBackupExplanation}</p>

<style>
  input:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

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
