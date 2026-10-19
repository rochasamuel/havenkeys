// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { en } from "../i18n/en";
import { CrashBoundary } from "./CrashBoundary";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function Broken(): never {
  throw new TypeError("Cannot read properties of undefined (reading 'length')");
}

afterEach(() => vi.restoreAllMocks());

it("shows what broke and a way back instead of a blank window", () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
  const host = document.createElement("div");
  const root = createRoot(host);
  act(() =>
    root.render(
      <CrashBoundary>
        <Broken />
      </CrashBoundary>,
    ),
  );
  expect(host.textContent).toContain(en.app.crashTitle);
  const details = host.querySelector(".crash-details")!.textContent!;
  expect(details).toContain("TypeError: Cannot read properties of undefined (reading 'length')");
  expect(details).toContain("Broken");
  expect(host.querySelector("button")!.textContent).toBe(en.app.crashReload);
  act(() => root.unmount());
});
