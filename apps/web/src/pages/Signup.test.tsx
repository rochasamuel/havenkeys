import type { ReactNode } from "react";
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ptBR } from "../i18n/pt-BR";
import { Signup, SignupView } from "./Signup";

const html = (node: ReactNode) => renderToString(<MemoryRouter>{node}</MemoryRouter>);

describe("Signup", () => {
  it("starts on the email step with the terms checkbox unchecked", () => {
    const h = html(<Signup />);
    expect(h).toContain('type="email"');
    expect(h).toContain('type="checkbox"');
    expect(h).not.toContain("checked");
    expect(h).toContain(en.signup.create);
  });

  it("renders the code step with a six-digit field and no resend before a minute", () => {
    const h = html(
      <SignupView
        state={{ step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: Date.now() }}
        now={Date.now()}
        dispatch={() => {}}
        t={en}
        locale="en"
      />,
    );
    expect(h).toContain('inputMode="numeric"');
    expect(h).toContain('maxLength="6"');
    expect(h).toContain("a@example.com");
    expect(h).toContain("disabled");
    expect(h).toContain(en.signup.resendIn.replace("{s}", "60"));
  });

  it("renders the invite on the done step in a read-only field with Copy", () => {
    const h = html(
      <SignupView state={{ step: "done", invite: "HKINV1-abcdef" }} now={0} dispatch={() => {}} t={ptBR} locale="pt-BR" />,
    );
    expect(h).toContain("HKINV1-abcdef");
    expect(h).toContain("readOnly");
    expect(h).toContain(ptBR.signup.copy);
    expect(h).toContain(ptBR.signup.alsoEmailed);
    expect(h).toContain(ptBR.common.installerWarning);
    expect(h).toContain(ptBR.common.betaNotice);
    expect(h).not.toContain("href=\"HKINV1");
  });

  it("says it is a beta on the first step", () => {
    expect(html(<Signup />)).toContain(en.common.betaNotice);
  });

  it("names each failure", () => {
    for (const error of ["rate_limited", "unavailable", "closed", "invalid", "network"] as const) {
      const h = html(
        <SignupView
          state={{ step: "email", email: "a@example.com", accepted: true, busy: false, error }}
          now={0}
          dispatch={() => {}}
          t={en}
          locale="en"
        />,
      );
      expect(h).toContain(en.signup.errors[error]);
    }
  });
});
