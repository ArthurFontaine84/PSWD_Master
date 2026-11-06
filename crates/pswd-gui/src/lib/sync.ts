import type { Disconnection, Exchange, SyncReport, SyncState } from "./api";
import { texts } from "./i18n";

/** Messages d'une opération avec le serveur : ce qui a été fait, puis ce qui a échoué. */
export interface Outcome {
  done: string[];
  problems: string[];
}

/** Synchros dont l'issue n'est signalée que si l'utilisateur l'a demandée. */
const QUIET: SyncState[] = ["synced", "notConnected"];

/** Résultat d'une synchro demandée par l'utilisateur : toujours un message. */
export function syncMessage(report: SyncReport): string {
  const detail = report.detail ?? "";
  switch (report.state) {
    case "synced":
      return texts.syncDone;
    case "notConnected":
      return texts.syncNotConnected;
    case "removed":
      return texts.syncRemoved;
    case "left":
      return texts.syncLeft;
    case "offline":
      return texts.syncOffline(detail);
    case "conflict":
      return texts.syncConflict;
    case "rejected":
      return texts.syncRejected(detail);
    case "failed":
      return texts.syncFailed(detail);
  }
}

/** Problème de synchro à signaler ; `null` si tout va bien ou s'il n'y a rien à synchroniser. */
export function syncProblem(report: SyncReport): string | null {
  return QUIET.includes(report.state) ? null : syncMessage(report);
}

export function exchangeOutcome(exchange: Exchange): Outcome {
  const done = [
    ...exchange.downloaded.map(({ name, localName }) => texts.downloaded(name, localName)),
    ...exchange.published.map((name) => texts.published(name)),
    ...exchange.sent.map((name) => texts.sent(name)),
    ...exchange.received.map((name) => texts.received(name)),
    ...exchange.removed.map((name) => texts.removedByServer(name)),
    ...exchange.left.map((name) => texts.leftElsewhere(name)),
  ];
  const problems = [
    ...exchange.notDownloaded.map(({ name, reason }) => texts.notDownloaded(name, reason)),
    ...exchange.notPublished.map(({ name, reason }) => texts.notPublished(name, reason)),
    ...exchange.needsUnlock.map((name) => texts.needsUnlock(name)),
    ...exchange.notUpdated.map(({ name, reason }) => texts.notUpdated(name, reason)),
    ...exchange.notLeft.map(({ name, reason }) => texts.notLeft(name, reason)),
  ];
  if (done.length === 0 && problems.length === 0) {
    done.push(texts.nothingToExchange);
  }
  return { done, problems };
}

export function disconnectionOutcome(disconnection: Disconnection): Outcome {
  const done = [texts.disconnected, ...disconnection.removed.map((name) => texts.removedFromDevice(name))];
  const problems = disconnection.kept.map(({ name, reason }) => texts.keptOnDevice(name, reason));
  if (disconnection.revokeError !== null) {
    problems.unshift(texts.tokenNotRevoked(disconnection.revokeError));
  }
  return { done, problems };
}
