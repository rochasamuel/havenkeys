/** What a key press in the Quick Access window asks for (spec §3 "Keys"). */
export type QuickKeyAction = "password" | "username" | "totp" | "open" | "hide" | "up" | "down";

export interface KeyLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  isComposing?: boolean;
  /** 229: an input method took the key (WKWebView can send its commit Enter with isComposing false). */
  keyCode?: number;
}

/**
 * `null` leaves the key to the browser (typing, Cmd/Ctrl+A, text copy…).
 * `mac`: Cmd is the command key; elsewhere Ctrl. Any other modifier, or a
 * key pressed while an input method is composing, is never an action.
 */
export function quickKeyAction(e: KeyLike, mac: boolean): QuickKeyAction | null {
  if (e.isComposing || e.keyCode === 229) return null;
  const command = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  if (command) {
    if (e.altKey || e.shiftKey) return null;
    switch (e.key.toLowerCase()) {
      case "c":
        return "username";
      case "t":
        return "totp";
      case "o":
        return "open";
      default:
        return null;
    }
  }
  if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return null;
  switch (e.key) {
    case "Enter":
      return "password";
    case "Escape":
      return "hide";
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    default:
      return null;
  }
}
