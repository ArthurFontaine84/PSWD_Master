import { describe, expect, it } from "vitest";
import { cleanVaultName, connectionProblem, newPasswordProblem, newVaultProblem } from "./validation";

const STRONG = "Maître-2026!";
const TOO_SHORT = "Le mot de passe maître doit contenir au moins 12 caractères.";
const TOO_SIMPLE =
  "Le mot de passe maître doit mélanger au moins 3 types de caractères parmi : minuscules, majuscules, chiffres, symboles.";

describe("newVaultProblem", () => {
  it("accepts a valid form, with a free name", () => {
    for (const name of ["Perso-2_x", "Mon coffre", "Été", "a/b", "é".repeat(32), "🔒".repeat(32)]) {
      expect(newVaultProblem(name, STRONG, STRONG)).toBeNull();
    }
  });

  it("requires a vault name of 64 characters at most", () => {
    expect(newVaultProblem("", STRONG, STRONG)).toBe("Le nom du coffre est obligatoire.");
    for (const name of ["a".repeat(33), "🔒".repeat(33), "per\tso", "per\nso"]) {
      expect(newVaultProblem(name, STRONG, STRONG)).toBe(
        "Nom invalide : 32 caractères au plus, sans tabulation ni retour à la ligne.",
      );
    }
  });

  it("requires a strong, confirmed master password", () => {
    expect(newVaultProblem("perso", "", "")).toBe("Le mot de passe maître est obligatoire.");
    expect(newVaultProblem("perso", "maître", "maître")).toBe(TOO_SHORT);
    expect(newVaultProblem("perso", STRONG, "Maitre-2026!")).toBe(
      "Les deux saisies ne correspondent pas.",
    );
  });
});

describe("cleanVaultName", () => {
  it("removes the spaces around the name and keeps one Unicode form", () => {
    expect(cleanVaultName("  Mon coffre \t")).toBe("Mon coffre");
    expect(cleanVaultName("Été")).toBe("Été");
  });
});

describe("newPasswordProblem", () => {
  const problem = (password: string) => newPasswordProblem(password, password);

  it("accepts twelve characters of three kinds", () => {
    for (const password of [
      "abcdefghij1A",
      "abcdefghij1!",
      "abcdefghijA!",
      "ABCDEFGHIJ1!",
      "Abcdefghij1!",
      "une phrase de passe Longue",
    ]) {
      expect(problem(password), password).toBeNull();
    }
  });

  it("refuses short passwords, counting characters rather than code units", () => {
    expect(problem("Abcdefghi1!")).toBe(TOO_SHORT);
    expect(problem("éééééééé1A!")).toBe(TOO_SHORT);
    expect(problem("ééééééééé1A!")).toBeNull();
    expect(problem("😀😀😀😀😀😀😀😀😀a1")).toBe(TOO_SHORT);
    expect(problem("😀😀😀😀😀😀😀😀😀😀a1")).toBeNull();
  });

  it("refuses passwords with fewer than three kinds", () => {
    for (const password of [
      "abcdefghijkl",
      "ABCDEFGHIJKL",
      "123456789012",
      "!!!!!!!!!!!!",
      "abcdefghijk1",
      "abcdefghijkL",
      "abcdefghijk!",
      "ABCDEFGHIJK1",
      "ABCDEFGHIJK!",
      "12345678901!",
    ]) {
      expect(problem(password), password).toBe(TOO_SIMPLE);
    }
  });

  it("classifies accented letters, spaces and caseless letters like the backend", () => {
    expect(problem("éèàùâêîôûç1É")).toBeNull();
    expect(problem("éèàùâêîôûçÉÈ")).toBe(TOO_SIMPLE);
    expect(problem("abcdefghij 1")).toBeNull();
    expect(problem("密码密码密码密码密码a1")).toBeNull();
  });

  it("checks the confirmation last", () => {
    expect(newPasswordProblem("a", "b")).toBe(TOO_SHORT);
    expect(newPasswordProblem(STRONG, "autre")).toBe("Les deux saisies ne correspondent pas.");
  });
});

describe("connectionProblem", () => {
  it("requires the address, the key and the device name, in that order", () => {
    expect(connectionProblem(" ", " ", " ")).toBe("L'adresse du serveur est obligatoire.");
    expect(connectionProblem("pswd.local", "", "")).toBe("La clé de connexion est obligatoire.");
    expect(connectionProblem("pswd.local", "AAAA", " ")).toBe("Le nom de cet appareil est obligatoire.");
    expect(connectionProblem("pswd.local", "AAAA", "PC")).toBeNull();
  });
});
