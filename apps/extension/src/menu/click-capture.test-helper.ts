// Shared by the field-menu and "sign in with" balloon test suites
// (menu.test.ts, sso.test.ts). jsdom never marks a script-dispatched event
// isTrusted (real for every DOM implementation, not a jsdom quirk — see
// content/content.test.ts), so a handler gated on `e.isTrusted` cannot be
// exercised with a real dispatched click. This captures the 'click'
// listeners a row/button registers as they're registered, so the handler
// can be invoked directly with a trusted event object instead.
//
// Not a `*.test.ts` file: it exports a helper, not a suite, so vitest must
// not collect it as a test file (see vitest's default include glob).
// hygiene.test.ts's source scan also excludes `*.test-helper.ts`, since
// this is test support, not shipped code.

import { vi } from "vitest";

export function captureClicks(): Map<Element, (e: Partial<MouseEvent>) => void> {
  const handlers = new Map<Element, (e: Partial<MouseEvent>) => void>();
  const orig = EventTarget.prototype.addEventListener;
  vi.spyOn(EventTarget.prototype, "addEventListener").mockImplementation(function (
    this: EventTarget,
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ) {
    if (type === "click" && listener && this instanceof Element) handlers.set(this, listener as (e: Partial<MouseEvent>) => void);
    return orig.call(this, type, listener, options);
  });
  return handlers;
}
