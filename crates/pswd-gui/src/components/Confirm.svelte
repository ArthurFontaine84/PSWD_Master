<script lang="ts">
  import { texts } from "../lib/i18n";

  interface Props {
    title: string;
    message: string;
    /** Bouton qui exécute l'action. */
    action: string;
    /** Bouton qui y renonce. */
    dismiss?: string;
    busy: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }

  const { title, message, action, dismiss = texts.cancel, busy, onConfirm, onCancel }: Props = $props();
</script>

<h3>{title}</h3>
<p>{message}</p>
<div class="actions">
  <button type="button" onclick={onCancel}>{dismiss}</button>
  <button type="button" class="danger" disabled={busy} onclick={onConfirm}>{action}</button>
</div>
