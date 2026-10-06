// Hands a freshly imported frame page (menu, save, passkey, sign-in-with)
// its session token the way the content script does: a message from the
// embedding window (in jsdom, the window is its own parent). See
// receiveToken in common.ts.
//
// Not a `*.test.ts` file: it exports a helper, not a suite.

import { TOKEN_MESSAGE } from "../messaging/inline";

export function postToken(token: string): void {
  window.dispatchEvent(new MessageEvent("message", { data: { type: TOKEN_MESSAGE, token }, source: window }));
}
