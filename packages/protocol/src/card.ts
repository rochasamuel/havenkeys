// Card fill roles, brands and the IIN table (spec 2026-09-29-card-autofill
// §4, §6.1). CARD_ROLES and CARD_BRANDS mirror CardRole and CardBrandId in
// crates/havenkeys-protocol/src/message.rs, in order; IIN_RANGES copies
// IIN_RANGES in crates/havenkeys-core/src/card.rs row for row
// (card-parity.test.ts checks all three). brandOf is for display only (the
// save prompt's logo); what is saved uses Rust's detection.

export const CARD_ROLES = [
  "cardholderName",
  "cardholderGivenName",
  "cardholderFamilyName",
  "number",
  "verificationNumber",
  "expiryMonth",
  "expiryYear",
  "brand",
] as const;
export type CardRole = (typeof CARD_ROLES)[number];

export const CARD_BRANDS = ["visa", "mastercard", "amex", "elo", "hipercard", "diners", "discover", "jcb", "unionpay", "maestro", "other"] as const;
export type CardBrandId = (typeof CARD_BRANDS)[number];

export const CARD_BRAND_NAMES: Record<CardBrandId, string> = {
  visa: "Visa",
  mastercard: "Mastercard",
  amex: "American Express",
  elo: "Elo",
  hipercard: "Hipercard",
  diners: "Diners Club",
  discover: "Discover",
  jcb: "JCB",
  unionpay: "UnionPay",
  maestro: "Maestro",
  other: "Card",
};

export const MAX_CARD_FRAMES = 8;
export const MAX_CARD_ROLES = 8;
export const MAX_CARD_VALUE_BYTES = 1024;

export function isCardRole(v: unknown): v is CardRole {
  return typeof v === "string" && (CARD_ROLES as readonly string[]).includes(v);
}

export function isCardBrand(v: unknown): v is CardBrandId {
  return typeof v === "string" && (CARD_BRANDS as readonly string[]).includes(v);
}

export interface CardMatch {
  id: string;
  title: string;
  brand: CardBrandId | null;
  last4: string | null;
  /** YYYY-MM. */
  expiry: string | null;
}

export interface CardValue {
  role: CardRole;
  value: string;
}

export interface CardFrameRequest {
  url: string;
  roles: CardRole[];
}

export interface IinRange {
  brand: CardBrandId;
  lo: string;
  hi: string;
}

/**
 * Copied from crates/havenkeys-core/src/card.rs IIN_RANGES, same order
 * (card-parity.test.ts checks). card.rs is the authority.
 */
export const IIN_RANGES: readonly IinRange[] = [
  { brand: "elo", lo: "401178", hi: "401179" },
  { brand: "elo", lo: "431274", hi: "431274" },
  { brand: "elo", lo: "438935", hi: "438935" },
  { brand: "elo", lo: "451416", hi: "451416" },
  { brand: "elo", lo: "457393", hi: "457393" },
  { brand: "elo", lo: "457631", hi: "457632" },
  { brand: "elo", lo: "504175", hi: "504175" },
  { brand: "elo", lo: "506699", hi: "506778" },
  { brand: "elo", lo: "509000", hi: "509999" },
  { brand: "elo", lo: "627780", hi: "627780" },
  { brand: "elo", lo: "636297", hi: "636297" },
  { brand: "elo", lo: "636368", hi: "636368" },
  { brand: "elo", lo: "650031", hi: "650033" },
  { brand: "elo", lo: "650035", hi: "650051" },
  { brand: "elo", lo: "650057", hi: "650081" },
  { brand: "elo", lo: "650405", hi: "650439" },
  { brand: "elo", lo: "650485", hi: "650538" },
  { brand: "elo", lo: "650541", hi: "650598" },
  { brand: "elo", lo: "650700", hi: "650718" },
  { brand: "elo", lo: "650720", hi: "650727" },
  { brand: "elo", lo: "650901", hi: "650978" },
  { brand: "elo", lo: "651652", hi: "651704" },
  { brand: "elo", lo: "655000", hi: "655019" },
  { brand: "elo", lo: "655021", hi: "655058" },
  { brand: "hipercard", lo: "606282", hi: "606282" },
  { brand: "hipercard", lo: "384100", hi: "384100" },
  { brand: "hipercard", lo: "384140", hi: "384140" },
  { brand: "hipercard", lo: "384160", hi: "384160" },
  { brand: "hipercard", lo: "637095", hi: "637095" },
  { brand: "hipercard", lo: "637568", hi: "637568" },
  { brand: "hipercard", lo: "637599", hi: "637599" },
  { brand: "hipercard", lo: "637609", hi: "637609" },
  { brand: "hipercard", lo: "637612", hi: "637612" },
  { brand: "visa", lo: "4", hi: "4" },
  { brand: "mastercard", lo: "51", hi: "55" },
  { brand: "mastercard", lo: "2221", hi: "2720" },
  { brand: "amex", lo: "34", hi: "34" },
  { brand: "amex", lo: "37", hi: "37" },
  { brand: "diners", lo: "300", hi: "305" },
  { brand: "diners", lo: "3095", hi: "3095" },
  { brand: "diners", lo: "36", hi: "36" },
  { brand: "diners", lo: "38", hi: "39" },
  { brand: "discover", lo: "6011", hi: "6011" },
  { brand: "discover", lo: "644", hi: "649" },
  { brand: "discover", lo: "65", hi: "65" },
  { brand: "jcb", lo: "3528", hi: "3589" },
  { brand: "unionpay", lo: "62", hi: "62" },
  { brand: "unionpay", lo: "8100", hi: "8171" },
  { brand: "maestro", lo: "50", hi: "50" },
  { brand: "maestro", lo: "56", hi: "58" },
  { brand: "maestro", lo: "6304", hi: "6304" },
  { brand: "maestro", lo: "67", hi: "67" },
];

/** The brand of a number's prefix: the first row that holds it. */
export function brandOf(digits: string): CardBrandId | null {
  if (!/^[0-9]{1,19}$/.test(digits)) return null;
  for (const r of IIN_RANGES) {
    if (digits.length < r.lo.length) continue;
    const prefix = digits.slice(0, r.lo.length);
    if (prefix >= r.lo && prefix <= r.hi) return r.brand;
  }
  return null;
}

/** The card check digit (Luhn). */
export function luhnOk(digits: string): boolean {
  if (!/^[0-9]{8,19}$/.test(digits)) return false;
  let sum = 0;
  for (let i = 0; i < digits.length; i++) {
    let d = digits.charCodeAt(digits.length - 1 - i) - 48;
    if (i % 2 === 1) {
      d *= 2;
      if (d > 9) d -= 9;
    }
    sum += d;
  }
  return sum % 10 === 0;
}
