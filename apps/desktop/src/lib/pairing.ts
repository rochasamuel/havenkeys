/**
 * How long a code is shown, counted on this computer's clock from when Rust
 * returned it (the server's own TTL), so a skewed clock cannot cut it short.
 */
export const PAIRING_SECONDS = 120;

/** Whole seconds until `deadline` (Unix ms), never negative. */
export function secondsLeft(deadline: number, now: number): number {
  return Math.max(0, Math.ceil((deadline - now) / 1000));
}

/** A poll error that ends the pairing: no later answer can change it. */
export function isTerminalPollError(code: string | undefined): boolean {
  return code === "pairing_failed" || code === "pairing_gone";
}

export type PollState = "waiting" | "denied" | "expired" | "approved";

/** What the panel does after one claim answer. */
export function nextStep(state: PollState): "poll" | "denied" | "expired" | "done" {
  switch (state) {
    case "waiting":
      return "poll";
    case "approved":
      return "done";
    default:
      return state;
  }
}
