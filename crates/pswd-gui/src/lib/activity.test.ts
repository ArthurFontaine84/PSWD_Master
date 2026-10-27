import { describe, expect, it, vi } from "vitest";
import { throttle } from "./activity";

describe("throttle", () => {
  it("sends at most once per interval", () => {
    let time = 1_000;
    const send = vi.fn();
    const notify = throttle(send, 100, () => time);
    notify();
    notify();
    time = 1_099;
    notify();
    expect(send).toHaveBeenCalledTimes(1);
    time = 1_100;
    notify();
    notify();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it("uses the system clock by default", () => {
    const send = vi.fn();
    const notify = throttle(send, 60_000);
    notify();
    notify();
    expect(send).toHaveBeenCalledTimes(1);
  });
});
