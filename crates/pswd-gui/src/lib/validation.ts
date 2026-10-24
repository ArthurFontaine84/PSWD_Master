// Contrôles immédiats du formulaire de création ; le backend refait toutes les vérifications.
import { texts } from "./i18n";

const MAX_NAME_LENGTH = 64;
const NAME_PATTERN = /^[A-Za-z0-9_-]+$/;

/** Le premier problème du formulaire, ou `null` s'il peut être envoyé. */
export function newVaultProblem(name: string, password: string, confirmation: string): string | null {
  if (name === "") {
    return texts.validation.nameRequired;
  }
  if (name.length > MAX_NAME_LENGTH || !NAME_PATTERN.test(name)) {
    return texts.validation.nameInvalid;
  }
  return newPasswordProblem(password, confirmation);
}

/** Problème d'un nouveau mot de passe maître et de sa confirmation, ou `null`. */
export function newPasswordProblem(password: string, confirmation: string): string | null {
  if (password === "") {
    return texts.validation.passwordRequired;
  }
  if (password !== confirmation) {
    return texts.validation.passwordMismatch;
  }
  return null;
}
