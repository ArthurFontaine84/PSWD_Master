<script lang="ts">
  import { onMount } from "svelte";
  import { currentVault, getSettings, onVaultLocked, type VaultInfo } from "./lib/api";
  import { throttle } from "./lib/activity";
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
    screen = { kind: "list", notice: texts.lockedForInactivity };
  }

  // Une action de l'utilisateur repousse le verrouillage ; si le coffre s'est déjà verrouillé
  // (événement manqué), on revient à la liste.
  const signalActivity = throttle(() => {
    void currentVault().then((vault) => {
      if (vault === null && screen.kind !== "list") {
        showLocked();
      }
    });
  }, ACTIVITY_INTERVAL);

  function onActivity() {
    if (screen.kind === "open" || screen.kind === "recovery") {
      signalActivity();
    }
  }

  onMount(() => {
    // Sans réglages lisibles, le thème reste celui du système.
    getSettings().then(
      (settings) => {
        applyTheme(settings.theme);
      },
      () => undefined,
    );
    const unlisten = onVaultLocked(showLocked);
    return () => {
      void unlisten.then((stop) => {
        stop();
      });
    };
  });
</script>

<svelte:window onkeydown={onActivity} onpointerdown={onActivity} />

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
  main {
    max-width: 64rem;
    margin: 0 auto;
    padding: var(--space-l) var(--space-m);
  }
</style>
