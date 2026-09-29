// Display helpers for the Identity item (spec 2026-09-29-identity-item). The
// values themselves are validated in Rust; this only decides what to show.

import type { IdentityFields } from "./types";

export type IdentityKey = Exclude<keyof IdentityFields, "custom" | "notes">;
export type IdentitySectionId = "identification" | "documents" | "contact" | "address" | "internet";

export interface IdentityFieldDef {
  key: IdentityKey;
  /** masked: hidden until revealed; the rest pick the input type. */
  kind?: "masked" | "date" | "tel" | "email" | "url";
  /** Characters Rust accepts, for the input's maxLength. */
  max: number;
}

export const IDENTITY_SECTIONS: ReadonlyArray<{ id: IdentitySectionId; fields: IdentityFieldDef[] }> = [
  {
    id: "identification",
    fields: [
      { key: "firstName", max: 256 },
      { key: "middleName", max: 256 },
      { key: "lastName", max: 256 },
      { key: "gender", max: 256 },
      { key: "birthDate", kind: "date", max: 10 },
      { key: "occupation", max: 256 },
      { key: "company", max: 256 },
      { key: "jobTitle", max: 256 },
    ],
  },
  {
    id: "documents",
    fields: [
      { key: "cpf", kind: "masked", max: 64 },
      { key: "rg", kind: "masked", max: 64 },
      { key: "passport", kind: "masked", max: 64 },
      { key: "driversLicense", kind: "masked", max: 64 },
    ],
  },
  {
    id: "contact",
    fields: [
      { key: "email", kind: "email", max: 512 },
      { key: "mobilePhone", kind: "tel", max: 32 },
      { key: "homePhone", kind: "tel", max: 32 },
      { key: "workPhone", kind: "tel", max: 32 },
    ],
  },
  {
    id: "address",
    fields: [
      { key: "street", max: 256 },
      { key: "number", max: 32 },
      { key: "complement", max: 256 },
      { key: "neighborhood", max: 256 },
      { key: "city", max: 256 },
      { key: "state", max: 256 },
      { key: "postalCode", max: 32 },
      { key: "country", max: 256 },
    ],
  },
  {
    id: "internet",
    fields: [
      { key: "username", max: 512 },
      { key: "website", kind: "url", max: 2048 },
    ],
  },
];

const filled = (v: string | null | undefined): v is string => typeof v === "string" && v.trim() !== "";

/** The sections with at least one value, each with only its filled fields. */
export function visibleSections(fields: IdentityFields) {
  return IDENTITY_SECTIONS.map((s) => ({ id: s.id, fields: s.fields.filter((f) => filled(fields[f.key])) })).filter(
    (s) => s.fields.length > 0,
  );
}

/** Nothing filled in at all: no field, no custom field, no notes. */
export function isEmptyIdentity(fields: IdentityFields): boolean {
  return visibleSections(fields).length === 0 && (fields.custom ?? []).length === 0 && !filled(fields.notes);
}

/** The identity's title is its name; an unnamed one shows `fallback`. */
export function identityTitle(title: string, fallback: string): string {
  return title.trim() || fallback;
}

/** `YYYY-MM-DD` shown in the UI's locale; the raw text if it does not parse. */
export function formatBirthDate(iso: string, locale: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  const date = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" }).format(date);
}
