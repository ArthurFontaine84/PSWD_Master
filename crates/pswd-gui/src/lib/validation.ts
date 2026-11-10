// Contrôles immédiats du formulaire de création ; le backend refait toutes les vérifications.
import { texts } from "./i18n";

// Même règle que `pswd_core::is_valid_vault_name` (SPEC §7).
const MAX_NAME_LENGTH = 64;
/** La longueur d'un nom tient sur un octet dans une sauvegarde. */
const MAX_NAME_BYTES = 255;
const CONTROL = /\p{Cc}/u;

// Même règle que `pswd_app::check_master_password` (SPEC §9).
const MIN_PASSWORD_LENGTH = 12;
const MIN_CHARACTER_KINDS = 3;
/** Minuscule, majuscule, chiffre, puis tout le reste (symbole, espace, lettre sans casse). */
const CHARACTER_KINDS = [
  /\p{Lowercase}/u,
  /\p{Uppercase}/u,
  /[0-9]/,
  /[^\p{Lowercase}\p{Uppercase}0-9]/u,
];

/** Nom de coffre tel qu'il sera enregistré : sans espaces au début ni à la fin, et sous une seule
 * forme Unicode, pour que deux noms identiques à l'écran le soient aussi pour l'application. */
export function cleanVaultName(text: string): string {
  return text.trim().normalize("NFC");
}

/** Le premier problème du formulaire, ou `null` s'il peut être envoyé ; `name` est déjà nettoyé
 * par `cleanVaultName`. */
export function newVaultProblem(name: string, password: string, confirmation: string): string | null {
  if (name === "") {
    return texts.validation.nameRequired;
  }
  if (
    [...name].length > MAX_NAME_LENGTH ||
    new TextEncoder().encode(name).length > MAX_NAME_BYTES ||
    CONTROL.test(name)
  ) {
    return texts.validation.nameInvalid;
  }
  return newPasswordProblem(password, confirmation);
}

/** Champ manquant du formulaire de connexion à un serveur, ou `null`. */
export function connectionProblem(address: string, key: string, deviceName: string): string | null {
  if (address.trim() === "") {
    return texts.validation.addressRequired;
  }
  if (key.trim() === "") {
    return texts.validation.keyRequired;
  }
  if (deviceName.trim() === "") {
    return texts.validation.deviceNameRequired;
  }
  return null;
}

/** Problème d'un nouveau mot de passe maître et de sa confirmation, ou `null`. */
export function newPasswordProblem(password: string, confirmation: string): string | null {
  if (password === "") {
    return texts.validation.passwordRequired;
  }
  // En caractères, comme le backend : `length` compterait deux unités pour certains symboles.
  if ([...password].length < MIN_PASSWORD_LENGTH) {
    return texts.validation.passwordTooShort;
  }
  if (CHARACTER_KINDS.filter((kind) => kind.test(password)).length < MIN_CHARACTER_KINDS) {
    return texts.validation.passwordTooSimple;
  }
  if (password !== confirmation) {
    return texts.validation.passwordMismatch;
  }
  return null;
}
