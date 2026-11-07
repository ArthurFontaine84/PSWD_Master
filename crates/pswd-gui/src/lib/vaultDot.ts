import type { ServerStatus, VaultServerState } from "./api";

/** Pastille d'un coffre : son état vis-à-vis du serveur, ou `unreachable` si les échanges échouent. */
export type VaultDot = VaultServerState | "unreachable";

export type DotColor = "green" | "orange" | "red";

/**
 * Rouge : aucun lien avec le serveur, ou aucun échange possible ; orange : lié, mais les deux
 * copies diffèrent ; vert : identiques (SPEC §11).
 */
export const dotColor: Record<VaultDot, DotColor> = {
  synced: "green",
  outdated: "orange",
  needsUnlock: "orange",
  local: "red",
  removed: "red",
  unreachable: "red",
};

/** Sans serveur enregistré, aucune pastille ; s'il ne répond pas, tous les coffres sont en rouge. */
export function vaultDot(
  server: Pick<ServerStatus, "address" | "trouble">,
  state: VaultServerState | null | undefined,
): VaultDot | null {
  if (server.address === null) {
    return null;
  }
  if (server.trouble !== null) {
    return "unreachable";
  }
  return state ?? null;
}
