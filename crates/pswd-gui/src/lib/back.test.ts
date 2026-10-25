import { exit, onBackButtonPress } from "@tauri-apps/api/app";
import type { PluginListener } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { goBack, listenBackButton, onBack } from "./back";

vi.mock("@tauri-apps/api/app", () => ({
  exit: vi.fn(),
  onBackButtonPress: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
});

describe("back button", () => {
  it("is answered by the last screen first, then by the earlier ones", () => {
    const list = vi.fn(() => true);
    const detail = vi.fn(() => false);
    const stopList = onBack(list);
    const stopDetail = onBack(detail);
    goBack();
    expect(detail).toHaveBeenCalledTimes(1);
    expect(list).toHaveBeenCalledTimes(1);
    expect(exit).not.toHaveBeenCalled();
    stopDetail();
    stopList();
  });

  it("closes the application when no screen goes back", () => {
    const first = vi.fn(() => false);
    const stop = onBack(first);
    goBack();
    expect(exit).toHaveBeenCalledTimes(1);
    stop();
    stop();
    goBack();
    expect(first).toHaveBeenCalledTimes(1);
    expect(exit).toHaveBeenCalledTimes(2);
  });

  it("replaces the system behaviour until stopped", async () => {
    const unregister = vi.fn();
    vi.mocked(onBackButtonPress).mockResolvedValue({ unregister } as unknown as PluginListener);
    const stop = await listenBackButton();
    expect(onBackButtonPress).toHaveBeenCalledWith(goBack);
    expect(unregister).not.toHaveBeenCalled();
    stop();
    expect(unregister).toHaveBeenCalledTimes(1);
  });
});
