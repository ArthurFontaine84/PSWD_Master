<script lang="ts">
  import { dotColor, type VaultDot } from "../lib/vaultDot";

  interface Props {
    /** Le texte de l'état est donné par l'élément qui contient la pastille. */
    dot: VaultDot;
  }

  const { dot }: Props = $props();
</script>

<span class="dot {dotColor[dot]}" aria-hidden="true"></span>

<style>
  .dot {
    flex: none;
    display: inline-block;
    width: 0.75rem;
    height: 0.75rem;
    border-radius: 50%;
  }

  .green {
    background: var(--success);
  }

  .orange {
    background: var(--warning);
  }

  .red {
    background: var(--danger);
  }
</style>
