import { describe, expect, it } from "vitest";
import { newPasswordProblem, newVaultProblem } from "./validation";

describe("newVaultProblem", () => {
  it("accepts a valid form", () => {
    expect(newVaultProblem("Perso-2_x", "maître", "maître")).toBeNull();
    expect(newVaultProblem("a".repeat(64), "m", "m")).toBeNull();
  });

  it("requires a safe vault name", () => {
    expect(newVaultProblem("", "m", "m")).toBe("Le nom du coffre est obligatoire.");
    for (const name of ["coffre perso", "a/b", "é", "a".repeat(65)]) {
      expect(newVaultProblem(name, "m", "m")).toBe(
        "Nom invalide : lettres, chiffres, - et _ uniquement, 64 caractères au plus.",
      );
    }
  });

  it("requires a confirmed master password", () => {
    expect(newVaultProblem("perso", "", "")).toBe("Le mot de passe maître est obligatoire.");
    expect(newVaultProblem("perso", "maître", "maitre")).toBe(
      "Les deux saisies ne correspondent pas.",
    );
  });
});

describe("newPasswordProblem", () => {
  it("requires a confirmed password", () => {
    expect(newPasswordProblem("maître", "maître")).toBeNull();
    expect(newPasswordProblem("", "")).toBe("Le mot de passe maître est obligatoire.");
    expect(newPasswordProblem("a", "b")).toBe("Les deux saisies ne correspondent pas.");
  });
});
