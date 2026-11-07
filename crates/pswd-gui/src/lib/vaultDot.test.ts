import { describe, expect, it } from "vitest";
import { dotColor, vaultDot } from "./vaultDot";

const registered = { address: "pswd.local:8443", trouble: null };

describe("vaultDot", () => {
  it("shows nothing without a registered server", () => {
    expect(vaultDot({ address: null, trouble: null }, "synced")).toBeNull();
  });

  it("turns every vault red while the server does not answer", () => {
    const trouble = { since: 0, reason: "serveur injoignable" };
    expect(vaultDot({ ...registered, trouble }, "synced")).toBe("unreachable");
    expect(vaultDot({ ...registered, trouble }, undefined)).toBe("unreachable");
    expect(dotColor.unreachable).toBe("red");
  });

  it("follows the state of the vault otherwise", () => {
    expect(vaultDot(registered, "outdated")).toBe("outdated");
    expect(vaultDot(registered, null)).toBeNull();
    expect(vaultDot(registered, undefined)).toBeNull();
  });

  it("keeps red for the vaults without a link to the server", () => {
    expect([dotColor.local, dotColor.removed]).toEqual(["red", "red"]);
    expect([dotColor.outdated, dotColor.needsUnlock, dotColor.synced]).toEqual(["orange", "orange", "green"]);
  });
});
