<script lang="ts">
  import {
    accountStatus,
    getSettings,
    saveSettings,
    type AccountStatus,
    type Settings,
    type Theme,
  } from "../lib/api";
  import { errorMessage } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { applyTheme } from "../lib/theme";
  import { showToast } from "../lib/toast.svelte";

  interface Props {
    onClose: () => void;
    /** Ouvre le panneau d'administration du serveur. */
    onAdmin: (account: AccountStatus) => void;
  }

  const { onClose, onAdmin }: Props = $props();

  const themes: Theme[] = ["auto", "light", "dark"];

  // Seul le thème se règle ici ; les autres valeurs, renvoyées telles quelles, restent dans
  // `config.toml`.
  let settings = $state<Settings | null>(null);
  let problem = $state<string | null>(null);

  getSettings().then(
    (loaded) => (settings = loaded),
    (error: unknown) => (problem = errorMessage(error)),
  );

  /** Compte de l'appareil ; `null` sans serveur, ou si le serveur ne répond pas. */
  let account = $state<AccountStatus | null>(null);
  accountStatus().then(
    (loaded) => (account = loaded),
    () => undefined,
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (settings === null) {
      return;
    }
    try {
      settings = await saveSettings($state.snapshot(settings));
      applyTheme(settings.theme);
      problem = null;
      showToast(texts.settingsSaved);
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<section class="panel" aria-labelledby="settings-title">
  <h2 id="settings-title">{texts.settings}</h2>
  {#if settings}
    <form onsubmit={submit}>
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
  {#if account?.isAdmin}
    {@const admin = account}
    <div class="actions">
      <button type="button" onclick={() => onAdmin(admin)}>
        {texts.administration}
      </button>
    </div>
  {/if}
</section>
