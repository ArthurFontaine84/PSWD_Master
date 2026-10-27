import { describe, expect, it, vi } from "vitest";
import { pageCount, pageOf, pageSizeFor, watchWide, WIDE_QUERY } from "./layout";

function mediaQuery(matches: boolean) {
  let listener: ((event: MediaQueryListEvent) => void) | undefined;
  const query = {
    matches,
    addEventListener: vi.fn((_: string, added: (event: MediaQueryListEvent) => void) => {
      listener = added;
    }),
    removeEventListener: vi.fn(),
  };
  const matchMedia = vi.fn(() => query as unknown as MediaQueryList);
  const change = (wide: boolean) => listener?.({ matches: wide } as MediaQueryListEvent);
  return { view: { matchMedia }, query, change };
}

describe("watchWide", () => {
  it("reports the current width, then each change, until stopped", () => {
    const { view, query, change } = mediaQuery(true);
    const onChange = vi.fn();
    const stop = watchWide(onChange, view);
    expect(view.matchMedia).toHaveBeenCalledWith(WIDE_QUERY);
    expect(onChange).toHaveBeenLastCalledWith(true);
    change(false);
    expect(onChange).toHaveBeenLastCalledWith(false);
    stop();
    expect(query.removeEventListener).toHaveBeenCalledWith("change", query.addEventListener.mock.calls[0]?.[1]);
  });

  it("keeps the narrow layout when the media queries are unknown", () => {
    const onChange = vi.fn();
    watchWide(onChange, {})();
    watchWide(onChange)();
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("pagination", () => {
  it("fits a page in the window height", () => {
    expect(pageSizeFor(1080)).toBe(8);
    expect(pageSizeFor(800)).toBe(5);
    expect(pageSizeFor(400)).toBe(3);
  });

  it("counts the pages, with at least one", () => {
    expect(pageCount(0, 5)).toBe(1);
    expect(pageCount(5, 5)).toBe(1);
    expect(pageCount(6, 5)).toBe(2);
  });

  it("cuts out one page", () => {
    const items = [1, 2, 3, 4, 5, 6, 7];
    expect(pageOf(items, 0, 3)).toEqual([1, 2, 3]);
    expect(pageOf(items, 2, 3)).toEqual([7]);
  });
});
