<script lang="ts">
  import { texts } from "../lib/i18n";

  interface Props {
    recoveryKey: string;
    onDone: () => void;
  }

  const { recoveryKey, onDone }: Props = $props();
  let noted = $state(false);
</script>

<section class="panel" aria-labelledby="recovery-title">
  <h2 id="recovery-title">{texts.recoveryTitle}</h2>
  <p>{texts.recoveryExplanation}</p>
  <p class="key"><code>{recoveryKey}</code></p>
  <label class="check">
    <input type="checkbox" bind:checked={noted} />
    {texts.recoveryNoted}
  </label>
  <div class="actions">
    <button type="button" class="primary" disabled={!noted} onclick={onDone}>{texts.continue}</button>
  </div>
</section>
