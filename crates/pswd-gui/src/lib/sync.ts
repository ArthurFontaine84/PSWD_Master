import type { Disconnection, Exchange, SyncReport, SyncState } from "./api";
import { texts } from "./i18n";

/** Intervalle de l'échange automatique et de la vérification du coffre ouvert (SPEC §7). */
export const EXCHANGE_INTERVAL = 5_000;

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
    case "deleted":
      return texts.syncDeleted;
    case "nameTaken":
      return texts.syncNameTaken;
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

/**
 * Un seul coffre échangé est nommé ; plusieurs sont comptés en une phrase. Les problèmes et ce
 * qui demande l'attention (coffre renommé, supprimé ou quitté) restent un par un. Un coffre absent
 * du compte n'est pas cité : sa pastille le signale.
 */
export function exchangeOutcome(exchange: Exchange): Outcome {
  const each = [
    ...exchange.downloaded.map(({ name, localName }) => texts.downloaded(name, localName)),
    ...exchange.published.map((name) => texts.published(name)),
    ...exchange.sent.map((name) => texts.sent(name)),
    ...exchange.received.map((name) => texts.received(name)),
  ];
  const renamed = exchange.downloaded
    .filter(({ name, localName }) => name !== localName)
    .map(({ name, localName }) => texts.renamedOnDownload(name, localName));
  const counts = {
    downloaded: exchange.downloaded.length,
    published: exchange.published.length,
    sent: exchange.sent.length,
    received: exchange.received.length,
  };
  const done = [
    ...exchange.renamed.map(({ name, newName }) => texts.gaveWay(name, newName)),
    ...(each.length > 1 ? [texts.exchangeSummary(counts), ...renamed] : each),
    ...exchange.left.map((name) => texts.leftElsewhere(name)),
    ...exchange.deleted.map((name) => texts.deletedElsewhere(name)),
  ];
  const problems = [
    ...exchange.notDownloaded.map(({ name, reason }) => texts.notDownloaded(name, reason)),
    ...exchange.notPublished.map(({ name, reason }) => texts.notPublished(name, reason)),
    ...exchange.needsUnlock.map((name) => texts.needsUnlock(name)),
    ...exchange.notUpdated.map(({ name, reason }) => texts.notUpdated(name, reason)),
    ...exchange.notLeft.map(({ name, reason }) => texts.notLeft(name, reason)),
    ...exchange.notDeleted.map(({ name, reason }) => texts.notDeleted(name, reason)),
  ];
  if (done.length === 0 && problems.length === 0 && exchange.removed.length === 0) {
    done.push(texts.nothingToExchange);
  }
  return { done, problems };
}

/** Les coffres retirés et gardés sont comptés ; seul un coffre gardé sur erreur est nommé. */
export function disconnectionOutcome(disconnection: Disconnection): Outcome {
  const done = [texts.disconnected(disconnection.removed.length, disconnection.kept.length)];
  const problems = disconnection.kept.flatMap(({ name, reason }) =>
    reason === null ? [] : [texts.keptOnDevice(name, reason)],
  );
  if (disconnection.revokeError !== null) {
    problems.unshift(texts.tokenNotRevoked(disconnection.revokeError));
  }
  return { done, problems };
}
