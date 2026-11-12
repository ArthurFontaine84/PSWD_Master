<script lang="ts">
  import { texts } from "../lib/i18n";
  import { verbatim } from "../lib/keyboard";
  import { addTags, splitDraft, suggestTags } from "../lib/tags";

  interface Props {
    tags: string[];
    /** Tag en cours de saisie : enregistré avec les autres s'il n'a pas encore été ajouté. */
    draft: string;
    /** Tags déjà utilisés dans le coffre, à proposer. */
    known: string[];
  }

  let { tags = $bindable(), draft = $bindable(), known }: Props = $props();

  const suggestions = $derived(suggestTags(known, tags, draft));

  function add(text: string) {
    tags = addTags(tags, text);
    draft = "";
  }

  function typed() {
    ({ tags, draft } = splitDraft(tags, draft));
  }

  function keydown(event: KeyboardEvent) {
    // Entrée ajoute le tag au lieu d'envoyer le formulaire.
    if (event.key === "Enter") {
      event.preventDefault();
      add(draft);
    }
  }
</script>

<fieldset>
  <legend>{texts.tags}</legend>
  {#if tags.length > 0}
    <ul class="chips" aria-label={texts.chosenTags}>
      {#each tags as tag (tag)}
        <li>
          {tag}
          <button
            type="button"
            class="remove"
            aria-label={texts.removeTag(tag)}
            title={texts.removeTag(tag)}
            onclick={() => (tags = tags.filter((other) => other !== tag))}
          >
            ×
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="new">
    <input
      bind:value={draft}
      oninput={typed}
      onkeydown={keydown}
      aria-label={texts.newTag}
      placeholder={texts.newTag}
      enterkeyhint="done"
      {...verbatim}
    />
    <button type="button" disabled={draft.trim() === ""} onclick={() => add(draft)}>{texts.addTag}</button>
  </div>
  {#if suggestions.length > 0}
    <div class="suggestions" role="group" aria-label={texts.existingTags}>
      {#each suggestions as tag (tag)}
        <button type="button" class="suggestion" aria-label={texts.addExistingTag(tag)} onclick={() => add(tag)}>
          + {tag}
        </button>
      {/each}
    </div>
  {/if}
  <small>{texts.tagsHint}</small>
</fieldset>

<style>
  /* Une seule colonne, à la largeur du cadre : sans cela, la largeur naturelle du champ et du
     bouton élargit la colonne, et le texte d'aide et « Ajouter » débordent de la carte. */
  fieldset {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-s);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-m);
  }

  .chips,
  .suggestions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-s);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .chips li {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.125rem 0.25rem 0.125rem 0.625rem;
    border-radius: 999px;
    background: var(--accent-soft);
    overflow-wrap: anywhere;
  }

  .remove {
    width: 1.75rem;
    height: 1.75rem;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: none;
    box-shadow: none;
    font-weight: normal;
  }

  .new {
    display: flex;
    gap: var(--space-s);
  }

  .new input {
    flex: 1;
    min-width: 0;
  }

  .suggestion {
    padding: 0.25rem 0.75rem;
    border-radius: 999px;
    font-weight: normal;
  }

  small {
    color: var(--text-muted);
  }
</style>
