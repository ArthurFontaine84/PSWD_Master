<script lang="ts">
  import { importEntries } from "../lib/api";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { pickCsvFile } from "../lib/files";
  import { texts } from "../lib/i18n";

  interface Props {
    onImported: () => void;
    onClose: () => void;
    onExpired: () => void;
  }

  const { onImported, onClose, onExpired }: Props = $props();

  let problem = $state<string | null>(null);
  let result = $state<string | null>(null);

  async function choose() {
    problem = null;
    result = null;
    try {
      const path = await pickCsvFile(texts.importCsv);
      if (path === null) {
        return;
      }
      result = texts.imported(await importEntries(path));
      onImported();
    } catch (error) {
      if (isVaultLocked(error)) {
        onExpired();
      } else {
        problem = errorMessage(error);
      }
    }
  }
</script>

<h3>{texts.importCsv}</h3>
<p>{texts.importExplanation}</p>
{#if problem}
  <p role="alert">{problem}</p>
{/if}
{#if result}
  <p role="status">{result}</p>
{/if}
<div class="actions">
  <button type="button" onclick={onClose}>{texts.cancel}</button>
  <button type="button" class="primary" onclick={choose}>{texts.chooseFile}</button>
</div>
