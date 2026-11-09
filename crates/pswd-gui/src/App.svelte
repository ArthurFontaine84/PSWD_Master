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
    type VaultInfo,
  } from "./lib/api";
  import { throttle } from "./lib/activity";
  import { listenBackButton, onBack } from "./lib/back";
  import { texts } from "./lib/i18n";
  import { motion } from "./lib/motion";
  import { applyTheme } from "./lib/theme";
  import { showToast } from "./lib/toast.svelte";
  import type { VaultDot } from "./lib/vaultDot";
  import Toast from "./components/Toast.svelte";
  import Icon from "./components/Icon.svelte";
  import Admin from "./screens/Admin.svelte";
  import Backup from "./screens/Backup.svelte";
  import CreateVault from "./screens/CreateVault.svelte";
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
    | { kind: "admin"; address: string; publicAddress: string | null }
    | { kind: "settings" };

  /** Intervalle minimal entre deux signaux d'activité envoyés au backend. */
  const ACTIVITY_INTERVAL = 30_000;

  let screen = $state<Screen>({ kind: "list" });

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
      <h1>
        <span class="logo"><Icon name="shield" /></span>
        {texts.appTitle}
        <span class="version">{texts.appVersion(__APP_VERSION__)}</span>
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
          <button
            type="button"
            class="icon-button"
            aria-label={texts.server}
            title={texts.server}
            onclick={() => (screen = { kind: "server" })}
          >
            <Icon name="server" />
          </button>
          <button
            type="button"
            class="icon-button"
            aria-label={texts.settings}
            title={texts.settings}
            onclick={() => (screen = { kind: "settings" })}
          >
            <Icon name="settings" />
          </button>
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
          address={screen.address}
          publicAddress={screen.publicAddress}
          onClose={() => (screen = { kind: "settings" })}
        />
      {:else if screen.kind === "settings"}
        <Settings
          onClose={() => (screen = { kind: "list" })}
          onAdmin={(address, publicAddress) => (screen = { kind: "admin", address, publicAddress })}
        />
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
    margin-bottom: var(--space-m);
  }

  h1 {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    margin: 0;
    font-size: 1.25rem;
    letter-spacing: -0.01em;
  }

  .logo {
    display: inline-flex;
    padding: 0.375rem;
    border-radius: var(--radius);
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    color: var(--on-accent);
    box-shadow: 0 4px 14px var(--glow-1);
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
