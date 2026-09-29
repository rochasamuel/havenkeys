import { describe, expect, it } from "vitest";
import { formatBirthDate, identityTitle, isEmptyIdentity, visibleSections } from "./identity";

describe("visibleSections", () => {
  it("keeps only sections and fields with a value, in order", () => {
    const sections = visibleSections({ firstName: "Samuel", lastName: " ", city: "Brasília", cpf: null });
    expect(sections.map((s) => s.id)).toEqual(["identification", "address"]);
    expect(sections[0]?.fields.map((f) => f.key)).toEqual(["firstName"]);
  });

  it("marks documents as masked", () => {
    const [docs] = visibleSections({ cpf: "123" });
    expect(docs?.fields[0]?.kind).toBe("masked");
  });
});

describe("isEmptyIdentity", () => {
  it("is empty only with no value, custom field or notes", () => {
    expect(isEmptyIdentity({})).toBe(true);
    expect(isEmptyIdentity({ firstName: "  ", notes: "" })).toBe(true);
    expect(isEmptyIdentity({ email: "me@example.com" })).toBe(false);
    expect(isEmptyIdentity({ custom: [{ label: "PIN", value: "1", hidden: true }] })).toBe(false);
    expect(isEmptyIdentity({ notes: "hi" })).toBe(false);
  });
});

describe("identityTitle", () => {
  it("falls back when the name is empty", () => {
    expect(identityTitle("Samuel Rocha", "Identity")).toBe("Samuel Rocha");
    expect(identityTitle("  ", "Identidade")).toBe("Identidade");
  });
});

describe("formatBirthDate", () => {
  it("shows the date itself, whatever the local time zone", () => {
    expect(formatBirthDate("2000-04-20", "en-US")).toBe("Apr 20, 2000");
    expect(formatBirthDate("2000-04-20", "pt-BR")).toBe("20 de abr. de 2000");
  });
  it("leaves anything else as typed", () => {
    expect(formatBirthDate("20/04/2000", "en-US")).toBe("20/04/2000");
  });
});
