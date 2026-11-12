import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { everyExchange } from "./poll";

describe("everyExchange", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("runs the task every five seconds while the app is visible, until stopped", () => {
    const task = vi.fn();
    const stop = everyExchange(task);
    vi.advanceTimersByTime(4_999);
    expect(task).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(task).toHaveBeenCalledTimes(1);

    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    vi.advanceTimersByTime(5_000);
    expect(task).toHaveBeenCalledTimes(1);
    visibility.mockRestore();

    stop();
    vi.advanceTimersByTime(5_000);
    expect(task).toHaveBeenCalledTimes(1);
  });
});
