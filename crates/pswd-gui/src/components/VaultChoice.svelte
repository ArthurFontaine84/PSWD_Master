<script lang="ts">
  import { texts } from "../lib/i18n";

  interface Props {
    names: string[];
    label: string;
    /** Les coffres cochés. */
    selection: string[];
  }

  let { names, label, selection = $bindable() }: Props = $props();

  const allSelected = $derived(names.length > 0 && selection.length === names.length);

  function toggleAll() {
    selection = allSelected ? [] : [...names];
  }
</script>

<div class="choices" role="group" aria-label={label}>
  <label class="check all">
    <input type="checkbox" checked={allSelected} onchange={toggleAll} />
    {texts.selectAll}
  </label>
  {#each names as name (name)}
    <label class="check">
      <input type="checkbox" bind:group={selection} value={name} />
      {name}
    </label>
  {/each}
</div>

<style>
  .choices {
    display: grid;
    margin-bottom: var(--space-m);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--input-bg);
    overflow: hidden;
  }

  .choices label {
    padding: 0.625rem var(--space-m);
    cursor: pointer;
    overflow-wrap: anywhere;
  }

  .choices label + label {
    border-top: 1px solid var(--border);
  }

  .all {
    font-weight: 600;
    background: var(--accent-soft);
  }

  input[type="checkbox"] {
    accent-color: var(--accent);
  }
</style>
