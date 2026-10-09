// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MemoryRouter, useLocation, useNavigate } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/context";
import { localeFromPath } from "../i18n/locale";
import { Signup } from "./Signup";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const INVITE = "HKINV1-abcdefghijklmnop0123456789";

let root: Root;
let container: HTMLElement;
let navigateTo: (to: string) => void = () => {};
let signupCalls: Array<{ path: string; body: unknown }> = [];

function Harness() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  navigateTo = (to) => navigate(to);
  return (
    <I18nProvider locale={localeFromPath(pathname)}>
      <Signup />
    </I18nProvider>
  );
}

async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

function type(input: HTMLInputElement, value: string) {
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function submit(form: HTMLElement) {
  (form.querySelector('button[type="submit"]') as HTMLButtonElement).click();
}

beforeEach(() => {
  signupCalls = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url);
      if (u.includes("/v1/signup/start")) {
        signupCalls.push({ path: "start", body: JSON.parse(String(init?.body)) });
        return new Response(null, { status: 202 });
      }
      if (u.includes("/v1/signup/verify")) {
        signupCalls.push({ path: "verify", body: JSON.parse(String(init?.body)) });
        return new Response(JSON.stringify({ invite: INVITE }), { status: 200 });
      }
      return new Response("{}", { status: 404 });
    }),
  );
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.unstubAllGlobals();
});

async function renderAt(path: string) {
  await act(async () => {
    root.render(
      <MemoryRouter initialEntries={[path]}>
        <Harness />
      </MemoryRouter>,
    );
  });
}

async function reachCodeStep() {
  const email = container.querySelector('input[type="email"]') as HTMLInputElement;
  await act(async () => type(email, "a@example.com"));
  await act(async () => (container.querySelector('input[type="checkbox"]') as HTMLInputElement).click());
  await act(async () => submit(container));
  await flush();
}

describe("Signup flow", () => {
  it("a language switch drops the flow and does not send a second code", async () => {
    await renderAt("/signup");
    await reachCodeStep();
    expect(signupCalls.filter((c) => c.path === "start")).toHaveLength(1);
    expect(container.querySelector('input[autocomplete="one-time-code"]')).not.toBeNull();

    await act(async () => navigateTo("/pt-br/signup"));
    await flush();

    expect(signupCalls.filter((c) => c.path === "start")).toHaveLength(1);
    expect(container.querySelector('input[type="email"]')).not.toBeNull();
    expect(container.querySelector('input[autocomplete="one-time-code"]')).toBeNull();
  });

  it("keeps the invite in a read-only field and out of storage, the title and the URL", async () => {
    await renderAt("/signup");
    const title = document.title;
    const pathname = window.location.pathname;
    await reachCodeStep();

    const code = container.querySelector('input[autocomplete="one-time-code"]') as HTMLInputElement;
    await act(async () => type(code, "123456"));
    await act(async () => submit(container));
    await flush();

    expect(signupCalls.find((c) => c.path === "verify")?.body).toEqual({ email: "a@example.com", code: "123456" });
    const field = container.querySelector("input[readonly]") as HTMLInputElement;
    expect(field.value).toBe(INVITE);
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
    expect(document.title).toBe(title);
    expect(window.location.pathname).toBe(pathname);
  });
});
