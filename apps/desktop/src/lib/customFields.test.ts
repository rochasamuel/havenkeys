import { describe, expect, it } from "vitest";
import {
  addField,
  fromViews,
  moveField,
  moveSection,
  newField,
  newSection,
  placeField,
  removeField,
  sectionsInput,
  type EditSection,
} from "./customFields";
import { KEEP } from "./secretEdit";
import type { SectionView } from "./types";

const views: SectionView[] = [
  {
    id: "s1",
    title: "Bank",
    fields: [
      { id: "f1", label: "PIN", type: "password", hasValue: true },
      { id: "f2", label: "Token", type: "otp", hasOtp: false },
      { id: "f3", label: "Account", type: "text", value: "12345" },
    ],
  },
  {
    id: "s2",
    title: null,
    fields: [{ id: "f4", label: "Home", type: "address", parts: { city: "Recife" }, formatted: "Recife" }],
  },
];

const NAMES = {
  text: "Text",
  url: "URL",
  email: "Email",
  address: "Address",
  date: "Date",
  otp: "One-time password",
  password: "Password",
  phone: "Phone",
};

const keys = (s: EditSection[]) => s.map((x) => x.fields.map((f) => f.id ?? f.label));

describe("customFields", () => {
  it("never sends sections that were not loaded, so Rust keeps them", () => {
    expect(sectionsInput(null, NAMES)).toBeUndefined();
  });

  it("opens existing secrets as keep and empty ones as empty", () => {
    const s = fromViews(views);
    expect(s[0]!.fields[0]!.secret).toEqual(KEEP);
    expect(s[0]!.fields[1]!.secret).toEqual({ mode: "set", value: "" });
    expect(s[0]!.fields[2]!.text).toBe("12345");
    expect(s[1]!.fields[0]!.address).toEqual({ city: "Recife" });
    expect(s[1]!.title).toBe("");
  });

  it("builds the save payload with ids, trimmed labels and secret updates", () => {
    const s = fromViews(views);
    const out = sectionsInput(s, NAMES)!;
    expect(out[0]).toEqual({
      id: "s1",
      title: "Bank",
      fields: [
        { id: "f1", label: "PIN", value: { type: "password", value: { op: "keep" } } },
        { id: "f2", label: "Token", value: { type: "otp", value: { op: "clear" } } },
        { id: "f3", label: "Account", value: { type: "text", value: "12345" } },
      ],
    });
    expect(out[1]).toEqual({
      id: "s2",
      title: null,
      fields: [{ id: "f4", label: "Home", value: { type: "address", value: { city: "Recife" } } }],
    });
  });

  it("new fields have no id and go to the last section", () => {
    const s = addField(fromViews(views), newField("email", " Mail "));
    const out = sectionsInput(s, NAMES)!;
    expect(out[1]!.fields[1]).toEqual({ label: "Mail", value: { type: "email", value: "" } });
    expect(addField([], newField("text", "x"))).toHaveLength(1);
  });

  it("falls back to the type name for a blank label", () => {
    const out = sectionsInput(addField([], newField("otp", "  ")), NAMES)!;
    expect(out[0]!.fields[0]!.label).toBe("One-time password");
  });

  it("moves a field within a section and across section edges, keeping its secret", () => {
    const s = fromViews(views);
    const pin = s[0]!.fields[0]!.key;
    expect(keys(moveField(s, pin, 1))).toEqual([["f2", "f1", "f3"], ["f4"]]);
    const account = s[0]!.fields[2]!.key;
    const moved = moveField(s, account, 1);
    expect(keys(moved)).toEqual([["f1", "f2"], ["f3", "f4"]]);
    const back = moveField(moved, account, -1);
    expect(keys(back)).toEqual([["f1", "f2", "f3"], ["f4"]]);
    expect(moveField(s, pin, -1)).toBe(s);
    const pinToEnd = placeField(s, pin, s[1]!.key, null);
    expect(keys(pinToEnd)).toEqual([["f2", "f3"], ["f4", "f1"]]);
    expect(pinToEnd[1]!.fields[1]!.secret).toEqual(KEEP);
  });

  it("drops a field before another one", () => {
    const s = fromViews(views);
    const account = s[0]!.fields[2]!.key;
    const pin = s[0]!.fields[0]!.key;
    expect(keys(placeField(s, account, s[0]!.key, pin))).toEqual([["f3", "f1", "f2"], ["f4"]]);
  });

  it("moves and removes sections", () => {
    const s = [...fromViews(views), newSection("New")];
    expect(moveSection(s, s[2]!.key, -1).map((x) => x.title)).toEqual(["Bank", "New", ""]);
    expect(keys(removeField(s, s[0]!.fields[1]!.key))[0]).toEqual(["f1", "f3"]);
  });
});
