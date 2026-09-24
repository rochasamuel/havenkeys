// Entry point for the isolated-world passkey bridge (see bridge.ts).
import { startBridge } from "./bridge";

declare global {
  // Set in this isolated world; page script cannot see it.
  var __havenkeysPasskeyBridge: boolean | undefined;
}

if (!globalThis.__havenkeysPasskeyBridge) {
  globalThis.__havenkeysPasskeyBridge = true;
  startBridge();
}
