import type { ApiFailure } from "./api";

// Three screens, one reducer, no DOM: the page renders this and dispatches
// into it, and the API calls are effects that end in `sent`, `verified` or
// `failed`. The invite exists only in the `done` state, in memory.

export type SignupError = ApiFailure["kind"] | "email" | "terms" | "code";

export type SignupState =
  | { step: "email"; email: string; accepted: boolean; busy: boolean; error: SignupError | null }
  | { step: "code"; email: string; code: string; busy: boolean; error: SignupError | null; sentAt: number }
  | { step: "done"; invite: string };

export type SignupAction =
  | { type: "email"; value: string }
  | { type: "accept"; value: boolean }
  | { type: "submit_email" }
  | { type: "sent"; at: number }
  | { type: "failed"; error: SignupError }
  | { type: "code"; value: string }
  | { type: "submit_code" }
  | { type: "verified"; invite: string }
  | { type: "resend" }
  | { type: "reset" };

export const RESEND_AFTER_MS = 60_000;

export function initialSignup(): SignupState {
  return { step: "email", email: "", accepted: false, busy: false, error: null };
}

// The server normalizes too; this only keeps the obviously wrong from
// costing a round trip. One @, something on both sides, no spaces.
function looksLikeEmail(value: string): boolean {
  const at = value.indexOf("@");
  return at > 0 && at === value.lastIndexOf("@") && at < value.length - 1 && !/\s/.test(value) && value.length <= 254;
}

export function normalizeCode(raw: string): string {
  return raw.replace(/\D/g, "").slice(0, 6);
}

export function canResend(state: SignupState, now: number): boolean {
  return state.step === "code" && !state.busy && now - state.sentAt >= RESEND_AFTER_MS;
}

export function signupReducer(state: SignupState, action: SignupAction): SignupState {
  if (action.type === "reset") return initialSignup();
  switch (state.step) {
    case "email":
      switch (action.type) {
        case "email":
          return { ...state, email: action.value, error: null };
        case "accept":
          return { ...state, accepted: action.value, error: null };
        case "submit_email": {
          const email = state.email.trim().toLowerCase();
          if (!looksLikeEmail(email)) return { ...state, error: "email" };
          if (!state.accepted) return { ...state, error: "terms" };
          return { ...state, email, busy: true, error: null };
        }
        case "sent":
          return { step: "code", email: state.email, code: "", busy: false, error: null, sentAt: action.at };
        case "failed":
          return { ...state, busy: false, error: action.error };
        default:
          return state;
      }
    case "code":
      switch (action.type) {
        case "code":
          return { ...state, code: normalizeCode(action.value), error: null };
        case "submit_code":
          if (state.code.length !== 6) return { ...state, error: "code" };
          return { ...state, busy: true, error: null };
        case "verified":
          return { step: "done", invite: action.invite };
        case "failed":
          return { ...state, busy: false, error: action.error };
        case "resend":
          return { ...state, code: "", busy: true, error: null };
        case "sent":
          return { ...state, busy: false, error: null, sentAt: action.at };
        default:
          return state;
      }
    case "done":
      return state;
  }
}
