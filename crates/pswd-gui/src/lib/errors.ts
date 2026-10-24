import type { UiError } from "./api";
import { texts } from "./i18n";

type ErrorCode = keyof typeof texts.errors;

function isUiError(error: unknown): error is UiError {
  return typeof error === "object" && error !== null && "code" in error && "detail" in error;
}

function isKnownCode(code: string): code is ErrorCode {
  return Object.hasOwn(texts.errors, code);
}

/** Message à afficher pour une erreur reçue du backend, dans la langue de l'interface. */
export function errorMessage(error: unknown): string {
  if (!isUiError(error)) {
    return texts.errors.unexpected(String(error));
  }
  const detail = error.detail ?? "";
  if (isKnownCode(error.code)) {
    return texts.errors[error.code](detail);
  }
  return texts.errors.unexpected(detail || error.code);
}
