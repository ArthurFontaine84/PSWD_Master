<script lang="ts">
  import { csvTemplateToFile, importEntries } from "../lib/api";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { pickCsvDestination, pickCsvFile } from "../lib/files";
  import { texts } from "../lib/i18n";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    onImported: () => void;
    onClose: () => void;
    onExpired: () => void;
  }

  const { onImported, onClose, onExpired }: Props = $props();

  let problem = $state<string | null>(null);
  /** Le rappel de supprimer le fichier CSV reste affiché plus longtemps qu'une simple confirmation. */
  const IMPORTED_DURATION = 8_000;

  async function downloadTemplate() {
    problem = null;
    try {
      const path = await pickCsvDestination(texts.templateFileTitle, texts.templateFileName);
      if (path === null) {
        return;
      }
      await csvTemplateToFile(path);
      showToast(texts.templateSaved);
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function choose() {
    problem = null;
    try {
      const path = await pickCsvFile(texts.importCsv);
      if (path === null) {
        return;
      }
      showToast(texts.imported(await importEntries(path)), { duration: IMPORTED_DURATION });
      onImported();
      onClose();
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
<p>{texts.templateExplanation}</p>
<div class="actions">
  <button type="button" onclick={downloadTemplate}>{texts.downloadTemplate}</button>
</div>
<p role="note" class="notice">{texts.importSecurity}</p>
{#if problem}
  <p role="alert">{problem}</p>
{/if}
<div class="actions">
  <button type="button" onclick={onClose}>{texts.cancel}</button>
  <button type="button" class="primary" onclick={choose}>{texts.chooseFile}</button>
</div>
