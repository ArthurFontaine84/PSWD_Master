<script lang="ts">
  import { untrack } from "svelte";
  import type { EntryInput } from "../lib/api";
  import {
    MAX_LENGTH,
    MIN_LENGTH,
    formProblem,
    toInput,
    type FormValues,
    type PasswordMode,
  } from "../lib/entryForm";
  import { errorMessage, isVaultLocked } from "../lib/errors";
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import PasswordInput from "./PasswordInput.svelte";

  interface Props {
    initial: FormValues;
    /** `true` pour modifier une entrée : le type ne change plus, le mot de passe peut rester. */
    editing: boolean;
    onSubmit: (input: EntryInput) => Promise<void>;
    onCancel: () => void;
    onExpired: () => void;
  }

  const { initial, editing, onSubmit, onCancel, onExpired }: Props = $props();

  // Le formulaire part des valeurs initiales, puis suit seulement la saisie.
  const values = $state<FormValues>(untrack(() => ({ ...initial })));
  let problem = $state<string | null>(null);
  let busy = $state(false);

  const modes: PasswordMode[] = $derived(
    editing ? ["keep", "set", "generate", "clear"] : ["set", "generate"],
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    problem = formProblem(values);
    if (problem !== null) {
      return;
    }
    const input = toInput(values);
    // Le mot de passe saisi ne reste pas dans le champ une fois envoyé.
    values.password = "";
    busy = true;
    try {
      await onSubmit(input);
    } catch (error) {
      if (isVaultLocked(error)) {
        onExpired();
      } else {
        problem = errorMessage(error);
      }
    } finally {
      busy = false;
    }
  }
</script>

<form onsubmit={submit} novalidate>
  {#if !editing}
    <label>
      {texts.kind}
      <select bind:value={values.kind}>
        <option value="login">{texts.kinds.login}</option>
        <option value="note">{texts.kinds.note}</option>
      </select>
    </label>
  {/if}
  <label>
    {texts.title}
    <input bind:value={values.title} autocomplete="off" />
  </label>
  {#if values.kind === "login"}
    <label>
      {texts.username}
      <input bind:value={values.username} {...verbatim} />
    </label>
    <fieldset>
      <legend>{texts.password}</legend>
      <div class="modes">
        {#each modes as mode (mode)}
          <label class="check">
            <input type="radio" name="password-mode" value={mode} bind:group={values.passwordMode} />
            {texts.passwordModes[mode]}
          </label>
        {/each}
      </div>
      {#if values.passwordMode === "set"}
        <PasswordInput bind:value={values.password} label={texts.password} hiddenLabel autocomplete="new-password" />
      {:else if values.passwordMode === "generate"}
        <div class="generator">
          <label>
            {texts.length}
            <input type="number" min={MIN_LENGTH} max={MAX_LENGTH} bind:value={values.length} />
          </label>
          <label class="check">
            <input type="checkbox" bind:checked={values.symbols} />
            {texts.symbols}
          </label>
        </div>
      {/if}
    </fieldset>
    <label>
      {texts.urls}
      <textarea bind:value={values.urls} rows="2" {...verbatim}></textarea>
      <small>{texts.urlsHint}</small>
    </label>
  {/if}
  <label>
    {texts.notes}
    <textarea bind:value={values.notes} rows="4"></textarea>
  </label>
  <label>
    {texts.tags}
    <input bind:value={values.tags} {...verbatim} />
    <small>{texts.tagsHint}</small>
  </label>
  {#if problem}
    <p role="alert">{problem}</p>
  {/if}
  <div class="actions">
    <button type="button" onclick={onCancel}>{texts.cancel}</button>
    <button type="submit" class="primary" disabled={busy}>{busy ? texts.saving : texts.save}</button>
  </div>
</form>

<style>
  fieldset {
    display: grid;
    gap: var(--space-s);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-m);
  }

  .modes,
  .generator {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--space-m);
  }

  .generator input[type="number"] {
    width: 6rem;
  }
</style>
