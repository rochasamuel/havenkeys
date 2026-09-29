// Identity fill roles (spec 2026-09-29-identity-autofill §4). Mirrors
// IdentityRole in crates/havenkeys-protocol/src/message.rs, in the same
// order (identity-parity.test.ts checks).

export const IDENTITY_ROLES = [
  "fullName",
  "firstName",
  "middleName",
  "lastName",
  "email",
  "phone",
  "birthDate",
  "birthDay",
  "birthMonth",
  "birthYear",
  "company",
  "street",
  "number",
  "complement",
  "addressLine1",
  "addressLine2",
  "neighborhood",
  "city",
  "state",
  "postalCode",
  "country",
  "username",
  "cpf",
  "rg",
  "passport",
  "driversLicense",
] as const;

export type IdentityRole = (typeof IDENTITY_ROLES)[number];

/** Filled only after the user confirms them, and only on https pages. */
export const DOCUMENT_ROLES: readonly IdentityRole[] = ["cpf", "rg", "passport", "driversLicense"];

export const MAX_IDENTITY_ROLES = 40;
export const MAX_IDENTITY_VALUE_BYTES = 4096;

export function isIdentityRole(v: unknown): v is IdentityRole {
  return typeof v === "string" && (IDENTITY_ROLES as readonly string[]).includes(v);
}

export interface IdentityValue {
  role: IdentityRole;
  value: string;
}
