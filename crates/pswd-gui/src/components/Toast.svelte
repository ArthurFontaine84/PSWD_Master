<script lang="ts">
  import { fly } from "svelte/transition";
  import { motion } from "../lib/motion";
  import { toast } from "../lib/toast.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if toast.message}
  {#key toast.id}
    <p class="toast {toast.color}" role="status" transition:fly={{ y: 24, duration: motion(250) }}>
      {#if toast.color === "green"}
        <Icon name="check" />
      {/if}
      {toast.message}
    </p>
  {/key}
{/if}

<style>
  /* Bulle posée en bas de l'écran, au-dessus du contenu, hors de la zone des gestes système. */
  .toast {
    position: fixed;
    left: 50%;
    bottom: max(var(--space-l), calc(env(safe-area-inset-bottom) + var(--space-m)));
    z-index: 100;
    display: flex;
    align-items: center;
    gap: var(--space-s);
    width: max-content;
    max-width: min(32rem, calc(100vw - 2 * var(--space-m)));
    margin: 0;
    padding: 0.75rem 1.25rem;
    transform: translateX(-50%);
    border-radius: 1.5rem;
    background: var(--success);
    color: var(--on-success);
    font-weight: 600;
    box-shadow: var(--shadow-raised);
  }

  .orange {
    background: var(--warning);
    color: var(--on-warning);
  }

  .red {
    background: var(--danger);
    color: var(--on-danger);
  }
</style>
