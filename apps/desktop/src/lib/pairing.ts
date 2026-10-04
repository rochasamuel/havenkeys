/** Whole seconds until `expiresAt`, never negative; unreadable means expired. */
export function secondsLeft(expiresAt: string, now: number): number {
  const at = Date.parse(expiresAt);
  if (Number.isNaN(at)) return 0;
  return Math.max(0, Math.ceil((at - now) / 1000));
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
