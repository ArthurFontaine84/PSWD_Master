import { describe, expect, it } from "vitest";
import type { Exchange, SyncReport, SyncState } from "./api";
import { disconnectionOutcome, exchangeOutcome, syncMessage, syncProblem } from "./sync";

function report(state: SyncState, detail: string | null = null): SyncReport {
  return { vault: "perso", state, detail, changed: false, passwordChanged: false };
}

const nothing: Exchange = {
  downloaded: [],
  notDownloaded: [],
  published: [],
  notPublished: [],
  sent: [],
  received: [],
  needsUnlock: [],
  notUpdated: [],
  removed: [],
  left: [],
  notLeft: [],
};

describe("syncProblem", () => {
  it("is silent when there is nothing to report", () => {
    expect(syncProblem(report("synced"))).toBeNull();
    expect(syncProblem(report("notConnected"))).toBeNull();
  });

  it("explains each problem", () => {
    expect(syncProblem(report("offline", "serveur injoignable"))).toBe(
      "Synchro impossible : serveur injoignable. Les modifications seront publiées plus tard.",
    );
    expect(syncProblem(report("removed"))).toBe(
      "Coffre supprimé du serveur : il n'existe plus que sur cet appareil. Pour le republier, appuyez sur « Synchroniser ».",
    );
    expect(syncProblem(report("left"))).toBe(
      "Vous n'êtes plus membre de ce coffre : il a été supprimé de cet appareil.",
    );
    expect(syncProblem(report("conflict"))).toBe(
      "Le coffre a changé sur le serveur à chaque essai : synchronisez de nouveau.",
    );
    expect(syncProblem(report("rejected", "coffre altéré"))).toBe(
      "Copie du serveur refusée (coffre altéré) : elle n'est pas remplacée.",
    );
    expect(syncProblem(report("failed", "disque plein"))).toBe("Synchro impossible : disque plein.");
    expect(syncProblem(report("failed"))).toBe("Synchro impossible : .");
  });
});

describe("syncMessage", () => {
  it("always says what happened", () => {
    expect(syncMessage(report("synced"))).toBe("Coffre à jour.");
    expect(syncMessage(report("notConnected"))).toBe("Cet appareil n'est connecté à aucun serveur.");
    expect(syncMessage(report("conflict"))).toBe(
      "Le coffre a changé sur le serveur à chaque essai : synchronisez de nouveau.",
    );
  });
});

describe("exchangeOutcome", () => {
  it("says when there was nothing to exchange", () => {
    expect(exchangeOutcome(nothing)).toEqual({
      done: ["Coffres à jour."],
      problems: [],
    });
  });

  it("lists what was exchanged, then what failed", () => {
    const exchange: Exchange = {
      downloaded: [
        { name: "perso", localName: "perso" },
        { name: "travail", localName: "travail-0123abcd" },
      ],
      notDownloaded: [{ name: "famille", reason: "disque plein" }],
      published: ["notes"],
      notPublished: [{ name: "abime", reason: "réponse inattendue du serveur (400)" }],
      sent: ["banque"],
      received: ["maison"],
      needsUnlock: ["voiture"],
      notUpdated: [{ name: "jeux", reason: "fichier introuvable" }],
      removed: ["ancien"],
      left: ["club"],
      notLeft: [{ name: "asso", reason: "accès refusé" }],
    };
    expect(exchangeOutcome(exchange)).toEqual({
      done: [
        "Coffre « perso » téléchargé.",
        "Coffre « travail » téléchargé sous le nom « travail-0123abcd » : ce nom était déjà pris.",
        "Coffre « notes » publié.",
        "Coffre « banque » envoyé au serveur.",
        "Coffre « maison » mis à jour depuis le serveur.",
        "Coffre « ancien » supprimé du serveur : gardé sur cet appareil. Pour le republier, ouvrez-le puis appuyez sur « Synchroniser ».",
        "Coffre « club » supprimé de cet appareil : vous n'êtes plus membre de ce coffre.",
      ],
      problems: [
        "Coffre « famille » du serveur non téléchargé : disque plein.",
        "Coffre « abime » non publié : réponse inattendue du serveur (400).",
        "Le coffre « voiture » a été modifié ici et sur le serveur : déverrouillez-le pour terminer la synchro.",
        "Coffre « jeux » non synchronisé : fichier introuvable.",
        "Vous n'êtes plus membre du coffre « asso », mais il n'a pas pu être supprimé de cet appareil : accès refusé.",
      ],
    });
  });

  it("does not claim that nothing was exchanged when everything failed", () => {
    const failed = { ...nothing, notPublished: [{ name: "abime", reason: "refusé" }] };
    expect(exchangeOutcome(failed).done).toEqual([]);
  });
});

describe("disconnectionOutcome", () => {
  it("lists the removed vaults", () => {
    expect(disconnectionOutcome({ revokeError: null, removed: ["perso"], kept: [] })).toEqual({
      done: [
        "Appareil déconnecté du serveur.",
        "Coffre « perso » retiré de cet appareil ; il reste sur le serveur.",
      ],
      problems: [],
    });
  });

  it("explains why vaults were kept and the token not revoked", () => {
    const outcome = disconnectionOutcome({
      revokeError: "serveur injoignable",
      removed: [],
      kept: [
        { name: "modifie", reason: null },
        { name: "perso", reason: "serveur injoignable" },
      ],
    });
    expect(outcome.done).toEqual(["Appareil déconnecté du serveur."]);
    expect(outcome.problems).toEqual([
      "Le serveur n'a pas pu être prévenu (serveur injoignable) : révoquez cet appareil depuis le panneau d'administration.",
      "Coffre « modifie » gardé : des modifications ne sont pas encore sur le serveur.",
      "Coffre « perso » gardé : serveur injoignable.",
    ]);
  });
});
