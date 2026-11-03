// Bank boxes: agência, conta and their check digits (spec
// 2026-11-03-bank-account §5.1).
//
// The login classifier refuses any box bankRoleOf claims, so a bank sign-in's
// agência or conta is never taken for a username. "Conta" alone means nothing
// ("Entrar na sua conta"): it counts only beside a branch box. Pure reads of
// attributes compared against fixed keyword lists; nothing from the page is
// evaluated or inserted. Imports nothing from group.ts at runtime, which
// imports this module (the shared input filter is usable.ts).

import { cardKindOf } from "./card-kind";
import { otpScore } from "./classify";
import type { Env } from "./group";
import { labelText } from "./labels";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";
import { usableInputs } from "./usable";
import { BANK_ACCOUNT, BANK_BRANCH, BANK_DIGIT, BANK_NEGATIVE, OTP_THRESHOLD } from "./words.generated";

export type BankRole = "bankBranch" | "bankBranchDigit" | "bankAccount" | "bankAccountDigit";

/** What a box's form says around it: another box with a branch or account word, and the bank role just before it. */
export type BankGroupFacts = {
  groupHasBranch: boolean;
  groupHasAccount: boolean;
  prevRole: BankRole | null;
  /** The next box is a one-character box too: a split one-time code, not a check digit. */
  nextIsSingle?: boolean;
};

const TYPES = new Set(["text", "tel", "number"]);
/**
 * The autocomplete names HTML defines for a field that is not a bank box. Any
 * other token ("nope", "false", "chrome-off") names nothing and is neutral.
 */
const AC_REAL =
  /^(username|email|impp|tel(-\S+)?|cc-\S+|one-time-code|(current|new)-password|name|honorific-\S+|given-name|additional-name|family-name|nickname|organization(-title)?|street-address|address-line\d|address-level\d|postal-code|country(-name)?|bday(-\S+)?|sex|url|photo|transaction-\S+|webauthn)$/;
const DIGIT_OF = { bankBranch: "bankBranchDigit", bankAccount: "bankAccountDigit" } as const;

function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

/** The box's normalized words, or null when it can never be a bank box (rule 4, BANK_NEGATIVE). */
function wordsOf(el: HTMLInputElement): string | null {
  if (!TYPES.has(el.type) || el.disabled || el.readOnly) return null;
  if (attr(el, "autocomplete").toLowerCase().split(/\s+/).some((t) => AC_REAL.test(t))) return null;
  if (cardKindOf(el, false)) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  const all = `${attrs} ${text}`;
  return hasAny(all, BANK_NEGATIVE) ? null : all;
}

/** The box's bank role given its form's facts, or null. */
export function bankRoleOf(el: HTMLInputElement, g: BankGroupFacts): BankRole | null {
  const words = wordsOf(el);
  if (words === null) return null;
  const max = el.maxLength;
  const otp = otpScore({ type: el.type, autocomplete: [], attrs: words, text: "", maxLength: max, inputMode: attr(el, "inputmode").toLowerCase(), codeMask: false }, false);
  // Rule 3: a check digit right after a branch or account box. A digit word
  // counts on a box with no length limit; a longer box ("Conta com dígito",
  // maxlength 8) is the number itself.
  // Not the first box of a split one-time code.
  if ((g.prevRole === "bankBranch" || g.prevRole === "bankAccount") && !(max === 1 && g.nextIsSingle) && otp < OTP_THRESHOLD) {
    if ((max >= 1 && max <= 2) || (max < 0 && hasAny(words, BANK_DIGIT))) return DIGIT_OF[g.prevRole];
  }
  // Rule 1: a branch box looks numeric, or sits beside an account box.
  const numeric = el.type === "number" || el.type === "tel" || attr(el, "inputmode").toLowerCase() === "numeric" || (max >= 1 && max <= 8);
  if (hasAny(words, BANK_BRANCH) && (numeric || g.groupHasAccount)) return "bankBranch";
  // Rule 2: an account box only ever sits beside a branch box.
  if (hasAny(words, BANK_ACCOUNT) && g.groupHasBranch) return "bankAccount";
  return null;
}

/** The bank role of each box, its facts read per form in document order. */
export function bankRoles(els: readonly HTMLInputElement[]): Map<HTMLInputElement, BankRole> {
  const forms = new Map<HTMLFormElement | null, HTMLInputElement[]>();
  for (const el of els) {
    const boxes = forms.get(el.form);
    if (boxes) boxes.push(el);
    else forms.set(el.form, [el]);
  }
  const out = new Map<HTMLInputElement, BankRole>();
  for (const boxes of forms.values()) {
    const words = boxes.map(wordsOf);
    const says = (i: number, list: readonly string[]) => words[i] != null && hasAny(words[i] as string, list);
    const branches = boxes.filter((_, i) => says(i, BANK_BRANCH)).length;
    const accounts = boxes.filter((_, i) => says(i, BANK_ACCOUNT)).length;
    let prevRole: BankRole | null = null;
    boxes.forEach((el, i) => {
      const groupHasBranch = branches - (says(i, BANK_BRANCH) ? 1 : 0) > 0;
      const groupHasAccount = accounts - (says(i, BANK_ACCOUNT) ? 1 : 0) > 0;
      const nextIsSingle = boxes[i + 1]?.maxLength === 1;
      prevRole = bankRoleOf(el, { groupHasBranch, groupHasAccount, prevRole, nextIsSingle });
      if (prevRole) out.set(el, prevRole);
    });
  }
  return out;
}

/** Every bank box under `root`, in document order, among the inputs the login classifier reads. */
export function classifyBank(root: ParentNode, env: Env): Array<{ el: HTMLInputElement; role: BankRole }> {
  const els = usableInputs(root, env);
  const roles = bankRoles(els);
  return els.flatMap((el) => {
    const role = roles.get(el);
    return role ? [{ el, role }] : [];
  });
}
