import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { hideToast, showToast, toast, TOAST_DURATION } from "./toast.svelte";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  hideToast();
  vi.useRealTimers();
});

describe("toast", () => {
  it("shows a message, then hides it after the default duration", () => {
    showToast("Enregistré");
    expect(toast.message).toBe("Enregistré");
    expect(toast.color).toBe("green");
    vi.advanceTimersByTime(TOAST_DURATION - 1);
    expect(toast.message).toBe("Enregistré");
    vi.advanceTimersByTime(1);
    expect(toast.message).toBeNull();
  });

  it("restarts the delay and changes the id for each new message", () => {
    showToast("Premier", { duration: 1_000 });
    const first = toast.id;
    vi.advanceTimersByTime(800);
    showToast("Second", { duration: 1_000 });
    expect(toast.id).toBe(first + 1);
    vi.advanceTimersByTime(800);
    expect(toast.message).toBe("Second");
    vi.advanceTimersByTime(200);
    expect(toast.message).toBeNull();
  });

  it("takes the color of a vault dot", () => {
    showToast("Synchro en pause", { color: "red" });
    expect(toast.color).toBe("red");
    showToast("Enregistré");
    expect(toast.color).toBe("green");
  });

  it("can be hidden early", () => {
    showToast("Enregistré");
    hideToast();
    expect(toast.message).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });
});
