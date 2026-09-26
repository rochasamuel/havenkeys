import { ApiError } from "../lib/api";
import type { ErrorCode, Messages } from "./en";

/*
 * What the UI says when a command fails. Rust errors reach the UI as a
 * fixed `{ code, message }` (never user data), and Rust stays English: the
 * UI shows its own text for a known code, Rust's English message for a code
 * it does not know (a newer core), and a generic line for anything that did
 * not come from the core at all.
 */

const INVALID_INPUT = /^Invalid input: (.+)\.$/;

function isKnownCode(code: string, t: Messages): code is ErrorCode {
  return Object.hasOwn(t.errors.codes, code);
}

export function errorMessage(e: unknown, t: Messages, fallback: string = t.errors.generic): string {
  if (!(e instanceof ApiError)) return fallback;
  if (!isKnownCode(e.code, t)) return e.message;
  if (e.code === "invalid_input") {
    const detail = INVALID_INPUT.exec(e.message)?.[1];
    const sentence = detail !== undefined && Object.hasOwn(t.errors.invalidInput, detail) ? t.errors.invalidInput[detail] : undefined;
    return sentence ?? e.message;
  }
  return t.errors.codes[e.code] ?? e.message;
}
