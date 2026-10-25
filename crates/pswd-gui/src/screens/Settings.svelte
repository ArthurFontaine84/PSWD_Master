<script lang="ts">
  import { getSettings, saveSettings, type Settings, type Theme } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { applyTheme } from "../lib/theme";

  interface Props {
    onClose: () => void;
  }

  const { onClose }: Props = $props();

  const themes: Theme[] = ["auto", "light", "dark"];

  let settings = $state<Settings | null>(null);
  let problem = $state<string | null>(null);
  let saved = $state(false);

  getSettings().then(
    (loaded) => (settings = loaded),
    (error: unknown) => (problem = errorMessage(error)),
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (settings === null) {
      return;
    }
    saved = false;
    try {
      settings = await saveSettings($state.snapshot(settings));
      applyTheme(settings.theme);
      problem = null;
      saved = true;
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<section aria-labelledby="settings-title">
  <h2 id="settings-title">{texts.settings}</h2>
  {#if settings}
    <form onsubmit={submit}>
      <label>
        {texts.autoLock}
        <input type="number" min="0" max="1440" bind:value={settings.autoLockMinutes} />
      </label>
      <label>
        {texts.clipboardClear}
        <input type="number" min="5" max="300" bind:value={settings.clipboardClearSeconds} />
      </label>
      <label>
        {texts.backgroundLock}
        <input type="number" min="0" max="3600" bind:value={settings.backgroundLockSeconds} />
      </label>
      <label>
        {texts.theme}
        <select bind:value={settings.theme}>
          {#each themes as theme (theme)}
            <option value={theme}>{texts.themes[theme]}</option>
          {/each}
        </select>
      </label>
      {#if problem}
        <p role="alert">{problem}</p>
      {/if}
      {#if saved}
        <p role="status">{texts.settingsSaved}</p>
      {/if}
      <div class="actions">
        <button type="button" onclick={onClose}>{texts.back}</button>
        <button type="submit" class="primary">{texts.save}</button>
      </div>
    </form>
  {:else if problem}
    <p role="alert">{problem}</p>
    <div class="actions">
      <button type="button" onclick={onClose}>{texts.back}</button>
    </div>
  {:else}
    <p>{texts.loading}</p>
  {/if}
</section>
