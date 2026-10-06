// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { TOKEN_MESSAGE } from "../messaging/inline";
import { receiveToken } from "./common";

const TOKEN = "b".repeat(32);

afterEach(() => {
  vi.useRealTimers();
  location.hash = "";
});

/** EX-03: the token comes only from the embedding window, never the URL. */
describe("receiveToken", () => {
  it("takes the first well-formed token message from the embedding window", async () => {
    const got = receiveToken();
    window.dispatchEvent(new MessageEvent("message", { data: { type: TOKEN_MESSAGE, token: "short" }, source: window }));
    window.dispatchEvent(new MessageEvent("message", { data: { type: "other", token: TOKEN }, source: window }));
    window.dispatchEvent(new MessageEvent("message", { data: { type: TOKEN_MESSAGE, token: TOKEN }, source: window }));
    window.dispatchEvent(new MessageEvent("message", { data: { type: TOKEN_MESSAGE, token: "c".repeat(32) }, source: window }));
    await expect(got).resolves.toBe(TOKEN);
  });

  it("ignores another window and the URL, and gives up after a while", async () => {
    vi.useFakeTimers();
    location.hash = TOKEN;
    const other = document.createElement("iframe");
    document.body.append(other);
    const got = receiveToken();
    window.dispatchEvent(
      new MessageEvent("message", { data: { type: TOKEN_MESSAGE, token: TOKEN }, source: other.contentWindow }),
    );
    await vi.advanceTimersByTimeAsync(5000);
    await expect(got).resolves.toBeNull();
    other.remove();
  });
});
