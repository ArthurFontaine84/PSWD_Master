import { describe, expect, it } from "vitest";
import { errorMessage, isServerUnreachable, isVaultLocked } from "./errors";

describe("isVaultLocked", () => {
  it("recognizes only the locked vault error", () => {
    expect(isVaultLocked({ code: "vaultLocked", detail: null })).toBe(true);
    expect(isVaultLocked({ code: "wrongPassword", detail: null })).toBe(false);
    expect(isVaultLocked("vaultLocked")).toBe(false);
  });
});

describe("isServerUnreachable", () => {
  it("recognizes only the unreachable server error", () => {
    expect(isServerUnreachable({ code: "serverUnreachable", detail: null })).toBe(true);
    expect(isServerUnreachable({ code: "serverError", detail: "serveur injoignable" })).toBe(false);
    expect(isServerUnreachable("serverUnreachable")).toBe(false);
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
        " perso",
        "Nom de coffre invalide : «  perso » (32 caractères au plus, sans espace au début ni à la fin).",
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
      [
        "invalidUrl",
        null,
        "Adresse non ouverte : seules les adresses web (http ou https) s'ouvrent dans le navigateur.",
      ],
      ["urlNotOpened", "aucun navigateur", "Ouverture du navigateur impossible : aucun navigateur"],
      ["emptyTitle", null, "Le titre ne peut pas être vide."],
      ["emptyPassword", null, "Le mot de passe est obligatoire."],
      ["emptyNote", null, "La note est obligatoire."],
      ["invalidLength", null, "La longueur doit être comprise entre 8 et 24."],
      ["saveFailed", "disque plein", "Enregistrement impossible : disque plein"],
      ["wrongRecoveryKey", null, "Clé de récupération incorrecte."],
      [
        "invalidRecoveryKey",
        null,
        "Clé de récupération mal saisie : 13 groupes de 4 caractères séparés par des tirets.",
      ],
      ["confirmationMismatch", null, "Le nom retapé ne correspond pas."],
      ["fileUnreadable", "introuvable", "Fichier illisible : introuvable"],
      [
        "csvInvalidHeader",
        null,
        "Première ligne du CSV attendue : Titre / Title;Type / Kind;Nom d'utilisateur / Username;Mot de passe / Password;Urls;Notes;Tags",
      ],
      ["csvFieldCount", "3", "CSV, ligne 3 : nombre de colonnes incorrect."],
      ["csvUnknownKind", "4", "CSV, ligne 4 : type inconnu (login ou note attendu)."],
      ["csvInvalidUtf8", "5", "CSV, ligne 5 : texte qui n'est pas en UTF-8."],
      ["nothingToBackUp", null, "Aucun coffre à sauvegarder."],
      ["emptyBackup", null, "Cette sauvegarde ne contient aucun coffre."],
      ["notInBackup", "perso", "Cette sauvegarde ne contient pas de coffre « perso »."],
      ["invalidBackup", "altérée", "Sauvegarde inutilisable."],
      ["invalidSettings", null, "Réglages hors des valeurs permises."],
      ["ioError", "disque absent", "Erreur de fichier : disque absent"],
      ["notConnected", null, "Cet appareil n'est connecté à aucun serveur."],
      ["alreadyConnected", null, "Cet appareil est déjà connecté à un serveur."],
      ["invalidConnectionKey", null, "Clé de connexion mal saisie : 13 groupes de 4 caractères séparés par des tirets."],
      [
        "serverError",
        "serveur injoignable",
        "Échange avec le serveur impossible : serveur injoignable.",
      ],
      ["serverUnreachable", null, "Serveur injoignable."],
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
