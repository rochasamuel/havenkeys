import { describe, expect, it } from "vitest";
import { canResend, initialSignup, normalizeCode, RESEND_AFTER_MS, signupReducer, type SignupState } from "./signup";

function run(actions: Parameters<typeof signupReducer>[1][], from: SignupState = initialSignup()): SignupState {
  return actions.reduce(signupReducer, from);
}

describe("signup state", () => {
  it("validates the email and the terms before submitting", () => {
    const s = run([{ type: "email", value: "not an email" }, { type: "accept", value: true }, { type: "submit_email" }]);
    expect(s.step).toBe("email");
    expect(s.step === "email" && s.error).toBe("email");
    const t = run([{ type: "email", value: "a@example.com" }, { type: "submit_email" }]);
    expect(t.step === "email" && t.error).toBe("terms");
    const u = run([{ type: "email", value: " A@Example.com " }, { type: "accept", value: true }, { type: "submit_email" }]);
    expect(u.step === "email" && u.busy).toBe(true);
    expect(u.step === "email" && u.email).toBe("a@example.com");
  });

  it("moves to the code step when the code was sent", () => {
    const s = run([
      { type: "email", value: "a@example.com" },
      { type: "accept", value: true },
      { type: "submit_email" },
      { type: "sent", at: 1000 },
    ]);
    expect(s).toEqual({ step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: 1000 });
    expect(canResend(s, 1000 + RESEND_AFTER_MS - 1)).toBe(false);
    expect(canResend(s, 1000 + RESEND_AFTER_MS)).toBe(true);
  });

  it("code input is normalized to six digits", () => {
    expect(normalizeCode("123 456")).toBe("123456");
    expect(normalizeCode("12-34-56-78")).toBe("123456");
    expect(normalizeCode("abc")).toBe("");
    const s = run([{ type: "code", value: "12a3" }], { step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: 0 });
    expect(s.step === "code" && s.code).toBe("123");
    const t = run([{ type: "submit_code" }], s);
    expect(t.step === "code" && t.error).toBe("code");
    expect(t.step === "code" && t.busy).toBe(false);
  });

  it("errors are classified by code and clear on the next edit", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "123456", busy: true, error: null, sentAt: 0 };
    const limited = run([{ type: "failed", error: "rate_limited" }], code);
    expect(limited.step === "code" && limited.error).toBe("rate_limited");
    expect(limited.step === "code" && limited.busy).toBe(false);
    const cleared = run([{ type: "code", value: "1" }], limited);
    expect(cleared.step === "code" && cleared.error).toBeNull();
  });

  it("verified holds only the invite, and reset forgets everything", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "123456", busy: true, error: null, sentAt: 0 };
    const done = run([{ type: "verified", invite: "HKINV1-x" }], code);
    expect(done).toEqual({ step: "done", invite: "HKINV1-x" });
    expect(run([{ type: "reset" }], done)).toEqual(initialSignup());
  });

  it("resend goes back to busy on the code step and keeps the email", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "12", busy: false, error: "invalid", sentAt: 0 };
    const s = run([{ type: "resend" }], code);
    expect(s).toEqual({ step: "code", email: "a@example.com", code: "", busy: true, error: null, sentAt: 0 });
    const t = run([{ type: "sent", at: 5000 }], s);
    expect(t.step === "code" && t.sentAt).toBe(5000);
  });
});
