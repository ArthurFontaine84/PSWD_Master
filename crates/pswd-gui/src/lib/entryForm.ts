// Le formulaire d'une entrée : valeurs saisies, contrôles immédiats et conversion pour le
// backend, qui refait toutes les vérifications.
import type { EntryDetail, EntryInput, EntryKind } from "./api";
import { texts } from "./i18n";

export type PasswordMode = "keep" | "clear" | "set" | "generate";

export interface FormValues {
  kind: EntryKind;
  title: string;
  username: string;
  passwordMode: PasswordMode;
  /** Mot de passe saisi (mode `set`) : vidé dès l'envoi. */
  password: string;
  length: number;
  symbols: boolean;
  /** Une URL par ligne. */
  urls: string;
  notes: string;
  /** Séparés par des espaces ou des virgules. */
  tags: string;
}

export const MIN_LENGTH = 8;
export const MAX_LENGTH = 24;
const DEFAULT_LENGTH = 24;

/** Formulaire d'une nouvelle entrée. */
export function emptyForm(): FormValues {
  return {
    kind: "login",
    title: "",
    username: "",
    passwordMode: "set",
    password: "",
    length: DEFAULT_LENGTH,
    symbols: true,
    urls: "",
    notes: "",
    tags: "",
  };
}

/** Formulaire de modification : le mot de passe est gardé tant qu'on n'y touche pas. */
export function formFrom(detail: EntryDetail): FormValues {
  return {
    ...emptyForm(),
    kind: detail.kind,
    title: detail.title,
    username: detail.username ?? "",
    passwordMode: "keep",
    urls: detail.urls.join("\n"),
    notes: detail.notes ?? "",
    tags: detail.tags.join(" "),
  };
}

/** Le premier problème du formulaire, ou `null` s'il peut être envoyé. */
export function formProblem(values: FormValues): string | null {
  if (values.title.trim() === "") {
    return texts.validation.titleRequired;
  }
  const generating = values.kind === "login" && values.passwordMode === "generate";
  if (generating && !(Number.isInteger(values.length) && values.length >= MIN_LENGTH && values.length <= MAX_LENGTH)) {
    return texts.validation.lengthInvalid;
  }
  return null;
}

function words(text: string, separators: RegExp): string[] {
  return text.split(separators).filter((word) => word !== "");
}

/** Ce que reçoit le backend. */
export function toInput(values: FormValues): EntryInput {
  const password =
    values.passwordMode === "set"
      ? { mode: "set" as const, value: values.password }
      : values.passwordMode === "generate"
        ? { mode: "generate" as const, length: values.length, symbols: values.symbols }
        : { mode: values.passwordMode };
  return {
    kind: values.kind,
    title: values.title,
    username: values.username,
    password,
    urls: words(values.urls, /\s+/),
    notes: values.notes,
    tags: words(values.tags, /[\s,]+/),
  };
}
