/*
 * The editor's model of a login's custom fields (spec
 * 2026-09-30-login-custom-fields §5.3). Pure functions so the payload,
 * moves and drops are tested without React. Password and OTP values are
 * `SecretEdit`s: an existing one is `keep` and is never loaded to edit.
 */
import { EMPTY, KEEP, toUpdate, type SecretEdit } from "./secretEdit";
import type { AddressValue, FieldType, FieldValueInput, SectionInput, SectionView } from "./types";

/** The "add another field" menu, in 1Password's order. */
export const FIELD_TYPES: FieldType[] = ["text", "url", "email", "address", "date", "otp", "password", "phone"];
export const MAX_SECTIONS = 20;
export const MAX_FIELDS = 100;

export interface EditField {
  /** React key; never sent. */
  key: string;
  /** Absent for a field added in this edit. */
  id?: string;
  type: FieldType;
  label: string;
  /** text, url, email, phone, date */
  text: string;
  address: AddressValue;
  /** password, otp */
  secret: SecretEdit;
}

export interface EditSection {
  key: string;
  id?: string;
  title: string;
  fields: EditField[];
}

let counter = 0;
function newKey(): string {
  counter += 1;
  return `cf${counter}`;
}

export function fromViews(views: SectionView[]): EditSection[] {
  return views.map((s) => ({
    key: newKey(),
    id: s.id,
    title: s.title ?? "",
    fields: s.fields.map((f): EditField => {
      const base: EditField = { key: newKey(), id: f.id, type: f.type, label: f.label, text: "", address: {}, secret: EMPTY };
      switch (f.type) {
        case "password":
          return { ...base, secret: f.hasValue ? KEEP : EMPTY };
        case "otp":
          return { ...base, secret: f.hasOtp ? KEEP : EMPTY };
        case "address":
          return { ...base, address: { ...f.parts } };
        default:
          return { ...base, text: f.value };
      }
    }),
  }));
}

export function newField(type: FieldType, label: string): EditField {
  return { key: newKey(), type, label, text: "", address: {}, secret: EMPTY };
}

export function newSection(title = ""): EditSection {
  return { key: newKey(), title, fields: [] };
}

export function fieldCount(sections: EditSection[]): number {
  return sections.reduce((n, s) => n + s.fields.length, 0);
}

/** Adds to the last section, opening an untitled one when there is none. */
export function addField(sections: EditSection[], field: EditField): EditSection[] {
  if (sections.length === 0) return [{ ...newSection(), fields: [field] }];
  const last = sections.length - 1;
  return sections.map((s, i) => (i === last ? { ...s, fields: [...s.fields, field] } : s));
}

export function updateField(sections: EditSection[], key: string, change: Partial<EditField>): EditSection[] {
  return sections.map((s) =>
    s.fields.some((f) => f.key === key)
      ? { ...s, fields: s.fields.map((f) => (f.key === key ? { ...f, ...change } : f)) }
      : s,
  );
}

export function removeField(sections: EditSection[], key: string): EditSection[] {
  return sections.map((s) => ({ ...s, fields: s.fields.filter((f) => f.key !== key) }));
}

export function updateSection(sections: EditSection[], key: string, title: string): EditSection[] {
  return sections.map((s) => (s.key === key ? { ...s, title } : s));
}

export function removeSection(sections: EditSection[], key: string): EditSection[] {
  return sections.filter((s) => s.key !== key);
}

/**
 * One place up or down. At a section's edge the field moves into the
 * neighbouring section: its end going up, its start going down.
 * Returns `sections` itself when there is nowhere to go.
 */
export function moveField(sections: EditSection[], key: string, delta: -1 | 1): EditSection[] {
  const si = sections.findIndex((s) => s.fields.some((f) => f.key === key));
  if (si < 0) return sections;
  const fi = sections[si]!.fields.findIndex((f) => f.key === key);
  const target = fi + delta;
  const next = sections.map((s) => ({ ...s, fields: [...s.fields] }));
  const [field] = next[si]!.fields.splice(fi, 1);
  if (target >= 0 && target < sections[si]!.fields.length) {
    next[si]!.fields.splice(target, 0, field!);
    return next;
  }
  const ns = si + delta;
  if (ns < 0 || ns >= next.length) return sections;
  if (delta < 0) next[ns]!.fields.push(field!);
  else next[ns]!.fields.unshift(field!);
  return next;
}

export function moveSection(sections: EditSection[], key: string, delta: -1 | 1): EditSection[] {
  const i = sections.findIndex((s) => s.key === key);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= sections.length) return sections;
  const next = [...sections];
  [next[i], next[j]] = [next[j]!, next[i]!];
  return next;
}

/** Drag and drop: put the field before `beforeKey` in `sectionKey`, or at its end. */
export function placeField(
  sections: EditSection[],
  key: string,
  sectionKey: string,
  beforeKey: string | null,
): EditSection[] {
  const field = sections.flatMap((s) => s.fields).find((f) => f.key === key);
  if (!field || key === beforeKey) return sections;
  return removeField(sections, key).map((s) => {
    if (s.key !== sectionKey) return s;
    const at = beforeKey === null ? -1 : s.fields.findIndex((f) => f.key === beforeKey);
    const fields = [...s.fields];
    fields.splice(at < 0 ? fields.length : at, 0, field);
    return { ...s, fields };
  });
}

function valueInput(f: EditField): FieldValueInput {
  switch (f.type) {
    case "password":
    case "otp":
      return { type: f.type, value: toUpdate(f.secret) };
    case "address":
      return { type: "address", value: f.address };
    default:
      return { type: f.type, value: f.text };
  }
}

/** The save's `sections`. A blank label becomes the type's name, which the editor shows as its placeholder.
 * `null` (never loaded, or loading failed) sends nothing, so Rust keeps them. */
export function sectionsInput(
  sections: EditSection[] | null,
  typeNames: Record<FieldType, string>,
): SectionInput[] | undefined {
  if (sections === null) return undefined;
  return sections.map((s) => ({
    ...(s.id ? { id: s.id } : {}),
    title: s.title.trim() || null,
    fields: s.fields.map((f) => ({
      ...(f.id ? { id: f.id } : {}),
      label: f.label.trim() || typeNames[f.type],
      value: valueInput(f),
    })),
  }));
}
