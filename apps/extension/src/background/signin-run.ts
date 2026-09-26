// Automatic sign-in runs: which login the user picked in a tab, and which
// step of its sign-in comes next. Pure state: no chrome.*, no secrets.
//
// A run starts only from a pick (menu or popup) whose fill Rust marked
// autoSubmit, and only once the content script said it is pressing the
// page's button. It is bound to the tab, frame and exact origin of that
// pick, moves forward only (username → password → otp), accepts each step
// once, and lasts at most RUN_TTL_MS. Every value a continuation fills
// still comes from the desktop's origin-checked fill_item / get_totp.

import type { NextStep, RunStep } from "../messaging/inline";

export const RUN_TTL_MS = 2 * 60_000;

export interface RunFrame {
  tabId: number;
  frameId: number;
  origin: string;
}

export interface Run extends RunFrame {
  itemId: string;
  step: RunStep;
  hasTotp: boolean;
  expires: number;
}

export function nextStep(step: RunStep, hasTotp: boolean): NextStep | null {
  if (step === "username") return "password";
  if (step === "password") return hasTotp ? "otp" : null;
  return null;
}

export function createRuns(now: () => number) {
  const runs = new Map<number, Run>(); // by tab

  function get(tabId: number): Run | null {
    const r = runs.get(tabId);
    if (!r) return null;
    if (r.expires <= now()) {
      runs.delete(tabId);
      return null;
    }
    return r;
  }

  const sameFrame = (r: Run, f: RunFrame) => r.frameId === f.frameId && r.origin === f.origin;

  /** A pick's first step was pressed. Replaces the tab's run; null when that step was the last. */
  function start(frame: RunFrame, itemId: string, pressed: RunStep, hasTotp: boolean): Run | null {
    runs.delete(frame.tabId);
    if (nextStep(pressed, hasTotp) === null) return null;
    const run: Run = { ...frame, itemId, step: pressed, hasTotp, expires: now() + RUN_TTL_MS };
    runs.set(frame.tabId, run);
    return run;
  }

  /** cs_run_step: the run to continue (now at `kind`), or null to ignore the message. */
  function accept(frame: RunFrame, kind: NextStep): Run | null {
    const r = get(frame.tabId);
    if (!r || !sameFrame(r, frame) || nextStep(r.step, r.hasTotp) !== kind) return null;
    r.step = kind;
    return r;
  }

  /** cs_ready: what a (re)loaded frame should watch for. The run's frame on another origin ends it. */
  function watchFor(frame: RunFrame): NextStep | null {
    const r = get(frame.tabId);
    if (!r || r.frameId !== frame.frameId) return null;
    if (r.origin !== frame.origin) {
      runs.delete(frame.tabId);
      return null;
    }
    return nextStep(r.step, r.hasTotp);
  }

  /** cs_run_stop from the run's own frame. */
  function stop(frame: RunFrame): Run | null {
    const r = runs.get(frame.tabId);
    if (!r || !sameFrame(r, frame)) return null;
    runs.delete(frame.tabId);
    return r;
  }

  function end(tabId: number): Run | null {
    const r = runs.get(tabId) ?? null;
    runs.delete(tabId);
    return r;
  }

  function clear(): Run[] {
    const all = [...runs.values()];
    runs.clear();
    return all;
  }

  return { start, accept, watchFor, stop, end, clear, get };
}

export type Runs = ReturnType<typeof createRuns>;
