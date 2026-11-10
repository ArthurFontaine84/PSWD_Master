import { describe, expect, it } from "vitest";
import type { Exchange, SyncReport, SyncState } from "./api";
import { disconnectionOutcome, exchangeOutcome, syncMessage, syncProblem } from "./sync";

function report(state: SyncState, detail: string | null = null): SyncReport {
  return { vault: "perso", state, detail, changed: false, passwordChanged: false };
}

const nothing: Exchange = {
  renamed: [],
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
  deleted: [],
  notDeleted: [],
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
      "Coffre absent de votre compte sur le serveur : il n'existe que sur cet appareil. Pour le publier, appuyez sur « Synchroniser ».",
    );
    expect(syncProblem(report("left"))).toBe(
      "Vous n'êtes plus membre de ce coffre : il a été supprimé de cet appareil.",
    );
    expect(syncProblem(report("deleted"))).toBe(
      "Ce coffre a été supprimé partout depuis un autre appareil : il a été supprimé de cet appareil.",
    );
    expect(syncProblem(report("nameTaken"))).toBe(
      "Le serveur a déjà un coffre de ce nom : celui-ci sera renommé puis publié au retour à la liste des coffres.",
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
      renamed: [{ name: "banque", newName: "banque-2" }],
      downloaded: [
        { name: "perso", localName: "perso" },
        { name: "travail", localName: "travail-2" },
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
      deleted: ["vieux"],
      notDeleted: [{ name: "garde", reason: "fichier verrouillé" }],
    };
    expect(exchangeOutcome(exchange)).toEqual({
      done: [
        "Le serveur a déjà un coffre « banque » : celui de cet appareil devient « banque-2 ». S'il s'agit du même coffre, supprimez « banque-2 ».",
        "2 coffres téléchargés, 1 publié, 1 envoyé au serveur, 1 mis à jour depuis le serveur.",
        "Coffre « travail » enregistré sous le nom « travail-2 » : ce nom était déjà pris.",
        "Coffre « ancien » absent de votre compte sur le serveur : gardé sur cet appareil. Pour le publier, ouvrez-le puis appuyez sur « Synchroniser ».",
        "Coffre « club » supprimé de cet appareil : vous n'êtes plus membre de ce coffre.",
        "Coffre « vieux » supprimé de cet appareil : il a été supprimé partout depuis un autre appareil.",
      ],
      problems: [
        "Coffre « famille » du serveur non téléchargé : disque plein.",
        "Coffre « abime » non publié : réponse inattendue du serveur (400).",
        "Le coffre « voiture » a été modifié ici et sur le serveur : déverrouillez-le pour terminer la synchro.",
        "Coffre « jeux » non synchronisé : fichier introuvable.",
        "Vous n'êtes plus membre du coffre « asso », mais il n'a pas pu être supprimé de cet appareil : accès refusé.",
        "Le coffre « garde » a été supprimé partout, mais n'a pas pu être supprimé de cet appareil : fichier verrouillé.",
      ],
    });
  });

  it("names a single exchanged vault", () => {
    const one = (exchange: Partial<Exchange>) => exchangeOutcome({ ...nothing, ...exchange }).done;
    expect(one({ downloaded: [{ name: "perso", localName: "perso" }] })).toEqual(["Coffre « perso » téléchargé."]);
    expect(one({ downloaded: [{ name: "travail", localName: "travail-2" }] })).toEqual([
      "Coffre « travail » téléchargé sous le nom « travail-2 » : ce nom était déjà pris.",
    ]);
    expect(one({ published: ["notes"] })).toEqual(["Coffre « notes » publié."]);
    expect(one({ sent: ["banque"] })).toEqual(["Coffre « banque » envoyé au serveur."]);
    expect(one({ received: ["maison"] })).toEqual(["Coffre « maison » mis à jour depuis le serveur."]);
  });

  it("counts several exchanged vaults in one sentence", () => {
    const summary = (exchange: Partial<Exchange>) => exchangeOutcome({ ...nothing, ...exchange }).done;
    expect(summary({ published: ["a", "b", "c", "d"] })).toEqual(["4 coffres publiés."]);
    expect(summary({ downloaded: [{ name: "a", localName: "a" }], received: ["b", "c"] })).toEqual([
      "1 coffre téléchargé, 2 mis à jour depuis le serveur.",
    ]);
    expect(summary({ sent: ["a", "b"], published: ["c"] })).toEqual(["1 coffre publié, 2 envoyés au serveur."]);
  });

  it("lists the vaults absent from the account in one sentence", () => {
    expect(exchangeOutcome({ ...nothing, removed: ["perso", "travail"] }).done).toEqual([
      "Coffres absents de votre compte sur le serveur, gardés sur cet appareil : « perso », « travail ». Pour les publier, ouvrez-les puis appuyez sur « Synchroniser ».",
    ]);
  });

  it("does not claim that nothing was exchanged when everything failed", () => {
    const failed = { ...nothing, notPublished: [{ name: "abime", reason: "refusé" }] };
    expect(exchangeOutcome(failed).done).toEqual([]);
  });
});

describe("disconnectionOutcome", () => {
  it("reminds that the copies kept belong to this account", () => {
    expect(disconnectionOutcome({ revokeError: null, removed: [], kept: [] }, true).done).toEqual([
      "Appareil déconnecté du serveur.",
      "Les coffres gardés sur cet appareil restent liés à ce compte : s'ils appartiennent à quelqu'un d'autre, supprimez-les avant de connecter un autre compte.",
    ]);
  });

  it("lists the removed vaults", () => {
    expect(disconnectionOutcome({ revokeError: null, removed: ["perso"], kept: [] }, false)).toEqual({
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
    }, false);
    expect(outcome.done).toEqual([
      "Appareil déconnecté du serveur.",
      "Les coffres gardés sur cet appareil restent liés à ce compte : s'ils appartiennent à quelqu'un d'autre, supprimez-les avant de connecter un autre compte.",
    ]);
    expect(outcome.problems).toEqual([
      "Le serveur n'a pas pu être prévenu (serveur injoignable) : révoquez cet appareil depuis le panneau d'administration.",
      "Coffre « modifie » gardé : des modifications ne sont pas encore sur le serveur.",
      "Coffre « perso » gardé : serveur injoignable.",
    ]);
  });
});
