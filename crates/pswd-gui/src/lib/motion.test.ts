import { describe, expect, it, vi } from "vitest";
import { motion } from "./motion";

function view(reduce: boolean) {
  const matchMedia = vi.fn(() => ({ matches: reduce }) as MediaQueryList);
  return { matchMedia };
}

describe("motion", () => {
  it("keeps the duration when animations are allowed", () => {
    const allowed = view(false);
    expect(motion(250, allowed)).toBe(250);
    expect(allowed.matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
  });

  it("removes the animation when the system asks for less motion", () => {
    expect(motion(250, view(true))).toBe(0);
  });

  it("removes the animation when the media queries are unknown", () => {
    expect(motion(250, {})).toBe(0);
    expect(motion(250)).toBe(0);
  });
});
