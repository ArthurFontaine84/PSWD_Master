<script lang="ts">
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import Icon from "./Icon.svelte";

  interface Props {
    value: string;
    label: string;
    /** `true` : le libellé n'est pas affiché, seuls les lecteurs d'écran le reçoivent. */
    hiddenLabel?: boolean;
    hint?: string;
    autocomplete: "current-password" | "new-password";
    autofocus?: boolean;
  }

  let { value = $bindable(), label, hiddenLabel = false, hint, autocomplete, autofocus = false }: Props = $props();

  const id = $props.id();
  /** Le mot de passe n'apparaît en clair que sur demande de l'utilisateur. */
  let visible = $state(false);
</script>

<div class="field">
  {#if !hiddenLabel}
    <label for={id}>{label}</label>
  {/if}
  <div class="control">
    <!-- svelte-ignore a11y_autofocus -->
    <input
      {id}
      type={visible ? "text" : "password"}
      bind:value
      {...verbatim}
      {autocomplete}
      {autofocus}
      aria-label={hiddenLabel ? label : undefined}
    />
    <button
      type="button"
      class="toggle"
      aria-label={texts.showPassword}
      aria-pressed={visible}
      title={texts.showPassword}
      onclick={() => (visible = !visible)}
    >
      <Icon name={visible ? "eye-off" : "eye"} />
    </button>
  </div>
  {#if hint}
    <small>{hint}</small>
  {/if}
</div>

<style>
  .field {
    display: grid;
    gap: 0.375rem;
    font-weight: 500;
  }

  .control {
    position: relative;
    display: grid;
  }

  .control input {
    padding-right: 3rem;
  }

  .toggle {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 3rem;
    min-height: 0;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--text-muted);
  }

  .toggle[aria-pressed="true"] {
    color: var(--accent);
  }

  @media (hover: hover) {
    .toggle:hover:not(:disabled) {
      background: none;
      color: var(--accent);
    }
  }
</style>
