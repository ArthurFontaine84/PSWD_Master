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
    expect(syncProblem(report("removed"))).toBe("Absent de votre compte sur le serveur : « Synchroniser » le publie.");
    expect(syncProblem(report("left"))).toBe("Vous n'êtes plus membre : coffre retiré de cet appareil.");
    expect(syncProblem(report("deleted"))).toBe("Coffre supprimé depuis un autre appareil.");
    expect(syncProblem(report("nameTaken"))).toBe(
      "Nom déjà pris sur le serveur : ce coffre sera renommé au retour à la liste.",
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
        "« banque » renommé « banque-2 » : nom déjà pris sur le serveur.",
        "2 coffres téléchargés, 1 publié, 1 envoyé au serveur, 1 mis à jour depuis le serveur.",
        "« travail » enregistré sous « travail-2 » (nom déjà pris).",
        "« club » retiré : vous n'êtes plus membre.",
        "« vieux » supprimé depuis un autre appareil.",
      ],
      problems: [
        "Coffre « famille » du serveur non téléchargé : disque plein.",
        "Coffre « abime » non publié : réponse inattendue du serveur (400).",
        "« voiture » modifié ici et sur le serveur : déverrouillez-le.",
        "Coffre « jeux » non synchronisé : fichier introuvable.",
        "« asso » : vous n'êtes plus membre, mais sa suppression ici a échoué : accès refusé.",
        "« garde » supprimé ailleurs, mais sa suppression ici a échoué : fichier verrouillé.",
      ],
    });
  });

  it("names a single exchanged vault", () => {
    const one = (exchange: Partial<Exchange>) => exchangeOutcome({ ...nothing, ...exchange }).done;
    expect(one({ downloaded: [{ name: "perso", localName: "perso" }] })).toEqual(["Coffre « perso » téléchargé."]);
    expect(one({ downloaded: [{ name: "travail", localName: "travail-2" }] })).toEqual([
      "« travail » téléchargé sous « travail-2 » (nom déjà pris).",
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

  it("leaves the vaults absent from the account to their dot", () => {
    expect(exchangeOutcome({ ...nothing, removed: ["perso", "travail"] })).toEqual({ done: [], problems: [] });
  });

  it("does not claim that nothing was exchanged when everything failed", () => {
    const failed = { ...nothing, notPublished: [{ name: "abime", reason: "refusé" }] };
    expect(exchangeOutcome(failed).done).toEqual([]);
  });
});

describe("disconnectionOutcome", () => {
  const done = (removed: string[], kept: string[]) =>
    disconnectionOutcome({ revokeError: null, removed, kept: kept.map((name) => ({ name, reason: null })) });

  it("counts the removed and kept vaults in one sentence", () => {
    expect(done([], [])).toEqual({ done: ["Appareil déconnecté."], problems: [] });
    expect(done(["a"], []).done).toEqual(["Appareil déconnecté : 1 coffre retiré."]);
    expect(done(["a", "b", "c"], []).done).toEqual(["Appareil déconnecté : 3 coffres retirés."]);
    expect(done([], ["a"]).done).toEqual(["Appareil déconnecté : 1 coffre gardé."]);
    expect(done([], ["a", "b"]).done).toEqual(["Appareil déconnecté : 2 coffres gardés."]);
    expect(done(["a", "b", "c"], ["d"]).done).toEqual(["Appareil déconnecté : 3 coffres retirés, 1 gardé."]);
    expect(done(["a"], ["b", "c"]).done).toEqual(["Appareil déconnecté : 1 coffre retiré, 2 gardés."]);
  });

  it("names only the vaults kept because of an error, and an unrevoked token", () => {
    const outcome = disconnectionOutcome({
      revokeError: "serveur injoignable",
      removed: [],
      kept: [
        { name: "modifie", reason: null },
        { name: "perso", reason: "serveur injoignable" },
      ],
    });
    expect(outcome.done).toEqual(["Appareil déconnecté : 2 coffres gardés."]);
    expect(outcome.problems).toEqual([
      "Serveur non prévenu (serveur injoignable) : révoquez cet appareil dans l'administration.",
      "« perso » gardé : serveur injoignable.",
    ]);
  });
});
