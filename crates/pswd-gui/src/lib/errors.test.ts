import { describe, expect, it } from "vitest";
import { errorMessage, isVaultLocked } from "./errors";

describe("isVaultLocked", () => {
  it("recognizes only the locked vault error", () => {
    expect(isVaultLocked({ code: "vaultLocked", detail: null })).toBe(true);
    expect(isVaultLocked({ code: "wrongPassword", detail: null })).toBe(false);
    expect(isVaultLocked("vaultLocked")).toBe(false);
  });
});

describe("errorMessage", () => {
  it("translates every known code", () => {
    const cases: [string, string | null, string][] = [
      ["configInvalid", "ligne 2", "Configuration invalide : ligne 2"],
      ["configUnreadable", "accès refusé", "Configuration illisible : accès refusé"],
      ["unknownVault", "perso", "Coffre inconnu : perso."],
      ["vaultExists", "perso", "Un coffre nommé perso existe déjà."],
      [
        "invalidVaultName",
        "a/b",
        "Nom de coffre invalide : « a/b » (lettres, chiffres, - et _ ; 64 caractères au plus).",
      ],
      ["passwordTooShort", null, "Le mot de passe maître doit contenir au moins 12 caractères."],
      [
        "passwordTooSimple",
        null,
        "Le mot de passe maître doit mélanger au moins 3 types de caractères parmi : minuscules, majuscules, chiffres, symboles.",
      ],
      ["wrongPassword", null, "Mot de passe maître incorrect."],
      ["vaultFolderExists", null, "Un dossier de coffre existe déjà à cet emplacement."],
      ["vaultUnreadable", "altéré", "Coffre illisible : altéré"],
      ["vaultLocked", null, "Le coffre est verrouillé."],
      ["entryNotFound", "1", "Cette entrée n'existe plus."],
      ["noPassword", "Wi-Fi", "L'entrée « Wi-Fi » n'a pas de mot de passe."],
      ["clipboard", "occupé", "Copie impossible : occupé"],
      ["emptyTitle", null, "Le titre ne peut pas être vide."],
      ["invalidLength", null, "La longueur doit être comprise entre 8 et 128."],
      ["saveFailed", "disque plein", "Enregistrement impossible : disque plein"],
      ["wrongRecoveryKey", null, "Clé de récupération incorrecte."],
      [
        "invalidRecoveryKey",
        null,
        "Clé de récupération mal saisie : 52 caractères, par groupes de 4.",
      ],
      ["confirmationMismatch", null, "Le nom retapé ne correspond pas."],
      ["fileUnreadable", "introuvable", "Fichier illisible : introuvable"],
      [
        "csvInvalidHeader",
        null,
        "Première ligne du CSV attendue : title,kind,username,password,urls,notes,tags",
      ],
      ["csvFieldCount", "3", "CSV, ligne 3 : nombre de colonnes incorrect."],
      ["csvUnknownKind", "4", "CSV, ligne 4 : type inconnu (login ou note attendu)."],
      ["csvInvalidUtf8", "5", "CSV, ligne 5 : texte qui n'est pas en UTF-8."],
      ["nothingToBackUp", null, "Aucun coffre à sauvegarder."],
      ["emptyBackup", null, "Cette sauvegarde ne contient aucun coffre."],
      ["notInBackup", "perso", "Cette sauvegarde ne contient pas de coffre « perso »."],
      ["invalidBackup", "altérée", "Sauvegarde inutilisable : altérée"],
      ["invalidSettings", null, "Réglages hors des valeurs permises."],
      ["ioError", "disque absent", "Erreur de fichier : disque absent"],
      ["unexpected", "disque plein", "Erreur : disque plein"],
    ];
    for (const [code, detail, message] of cases) {
      expect(errorMessage({ code, detail })).toBe(message);
    }
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
