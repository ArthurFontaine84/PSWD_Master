<script lang="ts">
  import { onMount } from "svelte";
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
  import { applyTheme } from "./lib/theme";
  import CreateVault from "./screens/CreateVault.svelte";
  import OpenVault from "./screens/OpenVault.svelte";
  import RecoveryKey from "./screens/RecoveryKey.svelte";
  import Restore from "./screens/Restore.svelte";
  import Settings from "./screens/Settings.svelte";
  import Unlock from "./screens/Unlock.svelte";
  import VaultList from "./screens/VaultList.svelte";

  type Screen =
    | { kind: "list"; notice?: string }
    | { kind: "create" }
    | { kind: "recovery"; vault: VaultInfo; recoveryKey: string }
    | { kind: "unlock"; name: string }
    | { kind: "open"; vault: VaultInfo }
    | { kind: "restore" }
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

<main>
  <h1>{texts.appTitle}</h1>
  {#if screen.kind === "list"}
    <VaultList
      notice={screen.notice}
      onOpen={(name) => (screen = { kind: "unlock", name })}
      onCreate={() => (screen = { kind: "create" })}
      onRestore={() => (screen = { kind: "restore" })}
      onSettings={() => (screen = { kind: "settings" })}
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
      onUnlocked={(vault) => (screen = { kind: "open", vault })}
      onCancel={() => (screen = { kind: "list" })}
    />
  {:else if screen.kind === "restore"}
    <Restore
      onRestored={(notice) => (screen = { kind: "list", notice })}
      onCancel={() => (screen = { kind: "list" })}
    />
  {:else if screen.kind === "settings"}
    <Settings onClose={() => (screen = { kind: "list" })} />
  {:else}
    {@const name = screen.vault.name}
    <OpenVault
      vault={screen.vault}
      onLocked={() => (screen = { kind: "list" })}
      onExpired={showLocked}
      onDeleted={() => (screen = { kind: "list", notice: texts.vaultDeleted(name) })}
    />
  {/if}
</main>

<style>
  /* Les marges laissent libres l'encoche et les barres du système (affichage bord à bord). */
  main {
    max-width: 64rem;
    margin: 0 auto;
    padding: max(var(--space-m), env(safe-area-inset-top)) max(var(--space-m), env(safe-area-inset-right))
      max(var(--space-m), env(safe-area-inset-bottom)) max(var(--space-m), env(safe-area-inset-left));
  }

  @media (min-width: 48rem) {
    main {
      padding-top: max(var(--space-l), env(safe-area-inset-top));
    }
  }
</style>
