import { describe, expect, it } from "vitest";
import { errorMessage } from "./errors";

describe("errorMessage", () => {
  it("translates known codes with their detail", () => {
    expect(errorMessage({ code: "configInvalid", detail: "ligne 2" })).toBe(
      "Configuration invalide : ligne 2",
    );
    expect(errorMessage({ code: "configUnreadable", detail: "accès refusé" })).toBe(
      "Configuration illisible : accès refusé",
    );
    expect(errorMessage({ code: "unexpected", detail: "disque plein" })).toBe(
      "Erreur : disque plein",
    );
  });

  it("falls back on the detail, then on the code, for an unknown code", () => {
    expect(errorMessage({ code: "nouveau", detail: "précision" })).toBe("Erreur : précision");
    expect(errorMessage({ code: "nouveau", detail: null })).toBe("Erreur : nouveau");
  });

  it("does not trust inherited properties as codes", () => {
    expect(errorMessage({ code: "toString", detail: null })).toBe("Erreur : toString");
  });

  it("shows anything else as text", () => {
    expect(errorMessage(new Error("panne"))).toBe("Erreur : Error: panne");
    expect(errorMessage(null)).toBe("Erreur : null");
  });
});
