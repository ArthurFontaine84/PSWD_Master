<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import {
    appHidden,
    appShown,
    currentVault,
    deviceKind,
    getSettings,
    onVaultLocked,
    type AccountStatus,
    type VaultInfo,
  } from "./lib/api";
  import { throttle } from "./lib/activity";
  import { listenBackButton, onBack } from "./lib/back";
  import { texts } from "./lib/i18n";
  import { motion } from "./lib/motion";
  import { applyTheme } from "./lib/theme";
  import { showToast } from "./lib/toast.svelte";
  import type { VaultDot } from "./lib/vaultDot";
  import { watchWide } from "./lib/layout";
  import AppMenu, { type MenuItem } from "./components/AppMenu.svelte";
  import Toast from "./components/Toast.svelte";
  import Icon from "./components/Icon.svelte";
  import Admin from "./screens/Admin.svelte";
  import Backup from "./screens/Backup.svelte";
  import CreateVault from "./screens/CreateVault.svelte";
  import Help from "./screens/Help.svelte";
  import OpenVault from "./screens/OpenVault.svelte";
  import RecoveryKey from "./screens/RecoveryKey.svelte";
  import Restore from "./screens/Restore.svelte";
  import Server from "./screens/Server.svelte";
  import Settings from "./screens/Settings.svelte";
  import Unlock from "./screens/Unlock.svelte";
  import VaultList from "./screens/VaultList.svelte";

  type Screen =
    | { kind: "list"; notice?: string }
    | { kind: "create" }
    | { kind: "recovery"; vault: VaultInfo; recoveryKey: string }
    | { kind: "unlock"; name: string; dot: VaultDot | null; shared: boolean }
    | { kind: "open"; vault: VaultInfo }
    | { kind: "backup" }
    | { kind: "restore" }
    | { kind: "server" }
    | { kind: "admin"; account: AccountStatus }
    | { kind: "settings" }
    | { kind: "help" };

  /** Intervalle minimal entre deux signaux d'activité envoyés au backend. */
  const ACTIVITY_INTERVAL = 30_000;

  let screen = $state<Screen>({ kind: "list" });

  // Écran étroit : seul « Nouveau coffre » reste visible, les autres outils passent dans un menu.
  let compact = $state(false);
  const tools: MenuItem[] = [
    { label: texts.server, icon: "server", onSelect: () => (screen = { kind: "server" }) },
    { label: texts.settings, icon: "settings", onSelect: () => (screen = { kind: "settings" }) },
    { label: texts.help, icon: "help", onSelect: () => (screen = { kind: "help" }) },
  ];

  function showLocked() {
    screen = { kind: "list", notice: texts.lockedAutomatically };
  }

  function showsVault() {
    return screen.kind === "open" || screen.kind === "recovery";
  }

  // Le coffre s'est verrouillé sans que l'événement soit reçu : on revient à la liste.
  function checkStillOpen(vault: VaultInfo | null) {
    if (vault === null && showsVault()) {
      showLocked();
    }
  }

  // Une action de l'utilisateur repousse le verrouillage.
  const signalActivity = throttle(() => {
    void currentVault().then(checkStillOpen);
  }, ACTIVITY_INTERVAL);

  function onActivity() {
    if (showsVault()) {
      signalActivity();
    }
  }

  // Sur mobile, l'arrière-plan rapproche le verrouillage ; au retour, le coffre a pu se fermer.
  function onVisibilityChange() {
    if (document.visibilityState === "hidden") {
      void appHidden();
    } else {
      void appShown().then(checkStillOpen);
    }
  }

  // Bouton retour : vers la liste des coffres. La clé de récupération, affichée une seule fois,
  // ne se quitte que par « Continuer » ; un coffre ouvert répond lui-même.
  function back(): boolean {
    switch (screen.kind) {
      case "list":
      case "open":
        return false;
      case "recovery":
        return true;
      case "admin":
        screen = { kind: "settings" };
        return true;
      default:
        screen = { kind: "list" };
        return true;
    }
  }

  onMount(() => {
    const stopBack = onBack(back);
    const stopWide = watchWide((wide) => (compact = !wide));
    const stopButton = deviceKind().then(
      (kind) => (kind === "mobile" ? listenBackButton() : undefined),
      () => undefined,
    );
    // Sans réglages lisibles, le thème reste celui du système.
    getSettings().then(
      (settings) => {
        applyTheme(settings.theme);
      },
      () => undefined,
    );
    const unlisten = onVaultLocked(showLocked);
    return () => {
      stopBack();
      stopWide();
      void unlisten.then((stop) => {
        stop();
      });
      void stopButton.then((stop) => stop?.());
    };
  });
</script>

<svelte:window onkeydown={onActivity} onpointerdown={onActivity} />
<svelte:document onvisibilitychange={onVisibilityChange} />

<main class:roomy={screen.kind !== "open"}>
  <!-- Un coffre ouvert garde tout l'écran pour ses entrées. -->
  {#if screen.kind !== "open"}
    <header class="app-bar">
      {#snippet brand()}
        <span class="logo"><Icon name="shield" /></span>
        {texts.appTitle}
        <span class="version">{texts.appVersion(__APP_VERSION__)}</span>
      {/snippet}
      <!-- La clé de récupération ne se quitte que par « Continuer » : le titre n'y mène nulle part. -->
      <h1>
        {#if screen.kind === "recovery"}
          {@render brand()}
        {:else}
          <button type="button" class="home" title={texts.home} onclick={() => (screen = { kind: "list" })}>
            {@render brand()}
          </button>
        {/if}
      </h1>
      {#if screen.kind === "list"}
        <div class="tools">
          <button
            type="button"
            class="icon-button"
            aria-label={texts.newVault}
            title={texts.newVault}
            onclick={() => (screen = { kind: "create" })}
          >
            <Icon name="plus" />
          </button>
          {#if compact}
            <AppMenu items={tools} />
          {:else}
            {#each tools as tool (tool.label)}
              <button
                type="button"
                class="icon-button"
                aria-label={tool.label}
                title={tool.label}
                onclick={tool.onSelect}
              >
                <Icon name={tool.icon} />
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </header>
  {/if}
  {#key screen.kind}
    <div in:fly={{ y: 16, duration: motion(300) }}>
      {#if screen.kind === "list"}
        <VaultList
          notice={screen.notice}
          onOpen={(name, dot, shared) => (screen = { kind: "unlock", name, dot, shared })}
          onBackup={() => (screen = { kind: "backup" })}
          onRestore={() => (screen = { kind: "restore" })}
        />
      {:else if screen.kind === "create"}
        <CreateVault
          onCreated={({ vault, recoveryKey }) => (screen = { kind: "recovery", vault, recoveryKey })}
          onCancel={() => (screen = { kind: "list" })}
        />
      {:else if screen.kind === "recovery"}
        {@const vault = screen.vault}
        <RecoveryKey recoveryKey={screen.recoveryKey} onDone={() => (screen = { kind: "open", vault })} />
      {:else if screen.kind === "unlock"}
        <Unlock
          name={screen.name}
          dot={screen.dot}
          shared={screen.shared}
          onUnlocked={(vault) => (screen = { kind: "open", vault })}
          onCancel={() => (screen = { kind: "list" })}
          onGone={() => (screen = { kind: "list" })}
        />
      {:else if screen.kind === "backup"}
        <Backup
          onBackedUp={(message) => {
            showToast(message);
            screen = { kind: "list" };
          }}
          onCancel={() => (screen = { kind: "list" })}
        />
      {:else if screen.kind === "restore"}
        <Restore
          onRestored={(message) => {
            showToast(message);
            screen = { kind: "list" };
          }}
          onCancel={() => (screen = { kind: "list" })}
        />
      {:else if screen.kind === "server"}
        <Server onClose={() => (screen = { kind: "list" })} />
      {:else if screen.kind === "admin"}
        <Admin
          address={screen.account.address}
          publicAddress={screen.account.publicAddress}
          accountId={screen.account.accountId}
          onClose={() => (screen = { kind: "settings" })}
          onDisconnected={() => (screen = { kind: "list" })}
        />
      {:else if screen.kind === "settings"}
        <Settings
          onClose={() => (screen = { kind: "list" })}
          onAdmin={(account) => (screen = { kind: "admin", account })}
        />
      {:else if screen.kind === "help"}
        <Help onClose={() => (screen = { kind: "list" })} />
      {:else}
        <OpenVault
          vault={screen.vault}
          onLocked={() => (screen = { kind: "list" })}
          onExpired={showLocked}
          onDeleted={(message) => {
            showToast(message);
            screen = { kind: "list" };
          }}
        />
      {/if}
    </div>
  {/key}
  <Toast />
</main>

<style>
  /* Les marges laissent libres l'encoche et les barres du système (affichage bord à bord). */
  /* Sur mobile, la barre d'état n'est pas toujours déclarée par `env()` : un minimum la laisse libre. */
  main {
    max-width: 64rem;
    margin: 0 auto;
    padding: max(3rem, calc(env(safe-area-inset-top) + var(--space-l))) max(var(--space-m), env(safe-area-inset-right))
      max(var(--space-m), env(safe-area-inset-bottom)) max(var(--space-m), env(safe-area-inset-left));
  }

  .app-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-m);
    margin-bottom: 2rem;
  }

  h1,
  .home {
    display: flex;
    align-items: center;
    gap: 0.625rem;
  }

  h1 {
    flex-shrink: 0;
    margin: 0;
    font-size: 1.125rem;
    letter-spacing: -0.01em;
    white-space: nowrap;
  }

  .home {
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    font-weight: inherit;
    letter-spacing: inherit;
  }

  .logo {
    display: inline-flex;
    padding: 0.375rem;
    border-radius: var(--radius);
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    color: var(--on-accent);
    box-shadow: 0 4px 14px var(--glow-1);
    transition:
      box-shadow 0.2s var(--ease),
      transform 0.2s var(--ease);
  }

  @media (hover: hover) {
    .home:hover {
      background: none;
      color: var(--accent);
    }

    .home:hover .logo {
      transform: translateY(-1px);
      box-shadow: 0 6px 18px var(--glow-2);
    }
  }

  .version {
    align-self: flex-end;
    margin-bottom: 0.2em;
    font-size: 0.75rem;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--text-muted);
  }

  .tools {
    display: flex;
    gap: var(--space-s);
  }

  @media (min-width: 48rem) {
    main {
      padding-top: max(var(--space-l), env(safe-area-inset-top));
    }

    h1 {
      font-size: 1.5rem;
    }
  }

  /* Très grand écran : le contenu s'élargit et descend vers le centre. Un coffre ouvert garde toute
     la hauteur pour ses entrées. */
  @media (min-width: 100rem) and (min-height: 56rem) {
    main {
      max-width: 80rem;
    }

    main.roomy {
      padding-top: 12vh;
    }
  }
</style>
