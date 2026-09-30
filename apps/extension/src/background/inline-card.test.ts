import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import type { BackgroundToContent } from "../messaging/inline";
import { cardRows, displayExpiry, isExpired } from "./card-rows";
import { createInlineHandler, type CardReport, type FrameNode, type FrameRef } from "./inline-handler";

const VISA = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const OLD = "11111111-2222-4333-8444-555555555555";
const SHOP = "https://shop.com/checkout";
const top: FrameRef = { tabId: 1, frameId: 0, url: SHOP, origin: "https://shop.com" };
const stripe = (frameId: number, topUrl = SHOP): FrameRef => ({
  tabId: 1,
  frameId,
  documentId: `doc-${frameId}`,
  url: `https://js.stripe.com/v3/elements-inner-${frameId}.html`,
  topUrl,
  origin: "https://js.stripe.com",
});
const cards = [
  { id: OLD, title: "Old Visa", brand: "visa" as const, last4: "0004", expiry: "2020-01" },
  { id: VISA, title: "Visa", brand: "visa" as const, last4: "1111", expiry: "2033-04" },
];

interface Opts {
  /** The tab's frames (webNavigation.getAllFrames). Default: the top frame and every reporting frame as its child. */
  tree?: FrameNode[] | null;
  /** The top frame's answer to bg_host_menu. */
  hostReply?: unknown;
}

function setup(answer: (r: Request) => unknown, reports: CardReport[] = [], opts: Opts = {}) {
  const requests: Request[] = [];
  const sent: Array<{ frameId: number; documentId?: string; msg: BackgroundToContent }> = [];
  let n = 0;
  const tree: FrameNode[] | null =
    opts.tree !== undefined
      ? opts.tree
      : [
          { frameId: 0, parentFrameId: -1, url: SHOP },
          ...[3, 4, ...reports.map((r) => r.frame.frameId)]
            .filter((id, i, all) => id !== 0 && all.indexOf(id) === i)
            .map((id) => {
              const r = reports.find((x) => x.frame.frameId === id);
              return { frameId: id, parentFrameId: 0, url: r ? `${r.frame.url}?x=1#y` : stripe(id).url, documentId: r?.frame.documentId ?? `doc-${id}` };
            }),
        ];
  const h = createInlineHandler({
    client: {
      request: (async (r: Request) => {
        requests.push(r);
        const a = answer(r);
        if (a instanceof Error) throw a;
        return a;
      }) as never,
    },
    sendToFrame: async (to, msg) => {
      const m = msg as BackgroundToContent;
      sent.push({ frameId: to.frameId, ...(to.documentId === undefined ? {} : { documentId: to.documentId }), msg: m });
      if (m.type === "bg_fill") return { filled: 1, pressing: null };
      if (m.type === "bg_host_menu") return "hostReply" in opts ? opts.hostReply : { ok: true };
      return undefined;
    },
    sendToTab: async (_tabId, msg) => {
      if (msg.type !== "bg_card_scan") return;
      for (const r of reports) void h.handleContent(r.frame, { type: "cs_card_fields", scan: msg.scan, roles: r.roles });
    },
    frames: async () => tree,
    wait: async () => {},
    now: () => Date.UTC(2026, 8, 29),
    newToken: () => (++n).toString(16).padStart(32, "0"),
  });
  return { h, requests, sent };
}

const findCards = (r: Request) =>
  r.type === "find_cards"
    ? { type: "find_cards", insecure: !(r.topUrl ?? r.url).startsWith("https:"), cards: r.url.includes("ads.") ? [] : cards }
    : r.type === "fill_card"
      ? { type: "fill_card", frames: r.frames.map(() => ({ values: [{ role: "number", value: "4111111111111111" }] })) }
      : new Error(`unexpected ${r.type}`);

/** Rust's frame rule, roughly: an ads frame is neither same-site nor a processor. */
const denyAds = (r: Request) => (r.type === "find_cards" && r.url.startsWith("https://ads.") ? new BridgeError("denied", "x") : findCards(r));

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("card rows", () => {
  it("shows MM/YY and puts expired cards last", () => {
    expect(displayExpiry("2033-04")).toBe("04/33");
    expect(displayExpiry(null)).toBeNull();
    expect(displayExpiry("2033-4")).toBeNull();
    expect(cardRows(cards, Date.UTC(2026, 8, 29)).map((c) => [c.title, c.expiry, c.expired])).toEqual([
      ["Visa", "04/33", false],
      ["Old Visa", "01/20", true],
    ]);
  });

  it("counts the expiry month itself as valid", () => {
    const now = new Date(2026, 8, 15).getTime(); // local September 2026
    expect(isExpired("2026-09", now)).toBe(false);
    expect(isExpired("2026-08", now)).toBe(true);
    expect(isExpired(null, now)).toBe(false);
  });
});

describe("card menu", () => {
  it("offers the cards with the top page's site, never numbers", async () => {
    const { h } = setup(findCards);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { ok: boolean; token: string; rows: number };
    expect(open).toMatchObject({ ok: true, rows: 2 });
    const view = await h.handleInline(1, { type: "menu_state", token: open.token });
    expect(view).toEqual({
      ok: true,
      value: {
        state: "cards",
        site: "shop.com",
        insecure: false,
        cards: [
          { id: VISA, title: "Visa", brand: "visa", last4: "1111", expiry: "04/33", expired: false },
          { id: OLD, title: "Old Visa", brand: "visa", last4: "0004", expiry: "01/20", expired: true },
        ],
      },
    });
  });

  it("explains an http page instead of filling", async () => {
    const { h } = setup(findCards);
    const open = (await h.handleContent({ ...top, url: "http://shop.com/", origin: "http://shop.com" }, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_state", token: open.token })).toMatchObject({ value: { state: "cards", insecure: true, cards: [] } });
  });

  it("shows the unlock row when locked, and nothing in a frame Rust denies", async () => {
    const locked = setup(() => new BridgeError("locked", "x"));
    const open = (await locked.h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await locked.h.handleInline(1, { type: "menu_state", token: open.token })).toEqual({ ok: true, value: { state: "locked" } });
    const denied = setup(() => new BridgeError("denied", "x"));
    expect(await denied.h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })).toEqual({ ok: false });
  });

  it("asks the top frame to host a menu opened in a processor frame", async () => {
    const { h, sent } = setup(findCards);
    const anchor = { top: 5, left: 5, width: 200, height: 30 };
    const open = await h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor });
    expect(open).toMatchObject({ ok: true, hosted: true });
    // Sent to the top frame only, naming the frame by the browser's id and (stripped) URL.
    expect(sent.at(-1)).toMatchObject({ frameId: 0, msg: { type: "bg_host_menu", frameId: 3, url: stripe(3).url, anchor, rows: 2 } });
    // Closing reaches both the frame and the host.
    await h.handleInline(1, { type: "menu_close", token: (open as { token: string }).token });
    expect(sent.filter((s) => s.msg.type === "bg_close_menu").map((s) => s.frameId).sort()).toEqual([0, 3]);
  });

  it("draws the menu in the frame itself when the top frame cannot find its iframe", async () => {
    const { h, sent } = setup(findCards, [], { hostReply: { ok: false } });
    const anchor = { top: 5, left: 5, width: 200, height: 30 };
    const open = (await h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor })) as { ok: boolean; hosted?: true; token: string };
    expect(open.ok).toBe(true);
    expect(open.hosted).toBeUndefined();
    await h.handleInline(1, { type: "menu_resize", token: open.token, height: 120 });
    expect(sent.find((s) => s.msg.type === "bg_resize_menu")?.frameId).toBe(3);
    // The top frame's close of a menu it does not host is ignored.
    await h.handleContent(top, { type: "cs_close_menu", token: open.token });
    expect(await h.handleInline(1, { type: "menu_state", token: open.token })).toMatchObject({ ok: true });
  });

  it("routes a hosted menu's resize to the top frame, and accepts its close", async () => {
    const { h, sent } = setup(findCards);
    const anchor = { top: 5, left: 5, width: 200, height: 30 };
    const open = (await h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor })) as { token: string };
    await h.handleInline(1, { type: "menu_resize", token: open.token, height: 120 });
    expect(sent.find((s) => s.msg.type === "bg_resize_menu")?.frameId).toBe(0);
    await h.handleContent(top, { type: "cs_close_menu", token: open.token });
    expect(await h.handleInline(1, { type: "menu_state", token: open.token })).toMatchObject({ ok: false });
  });

  it("opens no menu in a processor frame nested under a cross-site frame", async () => {
    const ad = { frameId: 5, parentFrameId: 0, url: "https://ads.example.net/slot" };
    const tree = [{ frameId: 0, parentFrameId: -1, url: SHOP }, ad, { frameId: 6, parentFrameId: 5, url: stripe(6).url, documentId: "doc-6" }];
    const { h, sent } = setup(denyAds, [], { tree });
    const anchor = { top: 5, left: 5, width: 200, height: 30 };
    expect(await h.handleContent(stripe(6), { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor })).toEqual({ ok: false });
    expect(sent.filter((s) => s.msg.type === "bg_host_menu")).toEqual([]);
  });

  it("opens no menu in a subframe when the frame tree is unknown", async () => {
    const { h } = setup(findCards, [], { tree: null });
    expect(await h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })).toEqual({ ok: false });
    // The top frame needs no tree.
    expect(await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })).toMatchObject({ ok: true });
  });
});

describe("card pick", () => {
  it("fills the clicked frame and the tab's other card frames with one fill_card", async () => {
    const { h, requests, sent } = setup(findCards, [
      { frame: stripe(3), roles: ["number"] },
      { frame: stripe(4), roles: ["expiryMonth", "expiryYear"] },
    ]);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA })).toEqual({ ok: true, value: null });
    const fill = requests.find((r) => r.type === "fill_card");
    expect(fill).toEqual({
      type: "fill_card",
      itemId: VISA,
      topUrl: SHOP,
      frames: [
        { url: SHOP, roles: ["cardholderName"] },
        { url: stripe(3).url, roles: ["number"] },
        { url: stripe(4).url, roles: ["expiryMonth", "expiryYear"] },
      ],
    });
    const fills = sent.filter((s) => s.msg.type === "bg_fill");
    expect(fills.map((s) => [s.frameId, (s.msg as { token: string | null }).token])).toEqual([[0, open.token], [3, null], [4, null]]);
    expect(fills.every((s) => (s.msg as { submit: boolean }).submit === false)).toBe(true);
  });

  it("gives each frame only its own values, pinned to the document that reported", async () => {
    const values: Record<string, Array<{ role: string; value: string }>> = {
      [stripe(3).url]: [{ role: "number", value: "4111111111111111" }],
      [stripe(4).url]: [{ role: "verificationNumber", value: "737" }],
    };
    const { h, sent } = setup(
      (r) => (r.type === "fill_card" ? { type: "fill_card", frames: r.frames.map((f) => ({ values: values[f.url] ?? [] })) } : findCards(r)),
      [
        { frame: stripe(3), roles: ["number"] },
        { frame: stripe(4), roles: ["verificationNumber"] },
      ],
    );
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    const fills = sent.filter((s) => s.msg.type === "bg_fill");
    // The top frame's answer was empty: nothing sent there.
    expect(fills.map((s) => s.frameId)).toEqual([3, 4]);
    const a = fills.find((s) => s.frameId === 3)!;
    const b = fills.find((s) => s.frameId === 4)!;
    expect(a.documentId).toBe("doc-3");
    expect(b.documentId).toBe("doc-4");
    expect(a.msg).toMatchObject({ origin: "https://js.stripe.com", fill: { kind: "card", values: [{ role: "number", value: "4111111111111111" }] } });
    expect(b.msg).toMatchObject({ fill: { kind: "card", values: [{ role: "verificationNumber", value: "737" }] } });
    expect(JSON.stringify(b.msg)).not.toContain("4111111111111111");
    expect(JSON.stringify(a.msg)).not.toContain("737");
  });

  it("leaves out a frame from another top page and a frame Rust denies", async () => {
    const ads: FrameRef = { tabId: 1, frameId: 5, url: "https://ads.example.net/x", topUrl: SHOP, origin: "https://ads.example.net" };
    const { h, requests } = setup(denyAds, [
      { frame: stripe(3, "https://other.com/"), roles: ["number"] },
      { frame: ads, roles: ["number"] },
    ]);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }] });
  });

  it("leaves out a processor frame nested under a cross-site frame (shop → ad → Stripe)", async () => {
    const tree = [
      { frameId: 0, parentFrameId: -1, url: SHOP },
      { frameId: 3, parentFrameId: 0, url: stripe(3).url, documentId: "doc-3" },
      { frameId: 5, parentFrameId: 0, url: "https://ads.example.net/slot?id=1" },
      { frameId: 6, parentFrameId: 5, url: stripe(6).url, documentId: "doc-6" },
    ];
    const { h, requests, sent } = setup(
      denyAds,
      [
        { frame: stripe(3), roles: ["expiryMonth"] },
        { frame: stripe(6), roles: ["number"] },
      ],
      { tree },
    );
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }, { url: stripe(3).url }] });
    expect(sent.filter((s) => s.msg.type === "bg_fill").map((s) => s.frameId)).toEqual([0, 3]);
  });

  it("keeps a processor frame nested under a same-site frame", async () => {
    const tree = [
      { frameId: 0, parentFrameId: -1, url: SHOP },
      { frameId: 5, parentFrameId: 0, url: "https://pay.shop.com/frame" },
      { frameId: 6, parentFrameId: 5, url: stripe(6).url, documentId: "doc-6" },
    ];
    const { h, requests } = setup(findCards, [{ frame: stripe(6), roles: ["number"] }], { tree });
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }, { url: stripe(6).url }] });
    // The same-site parent was asked about by URL with the top page, like any frame.
    expect(requests).toContainEqual({ type: "find_cards", url: "https://pay.shop.com/frame", topUrl: SHOP });
  });

  it("leaves out a frame whose document changed or that the tree does not know", async () => {
    const tree = [
      { frameId: 0, parentFrameId: -1, url: SHOP },
      { frameId: 3, parentFrameId: 0, url: stripe(3).url, documentId: "doc-3-new" },
    ];
    const { h, requests } = setup(
      findCards,
      [
        { frame: stripe(3), roles: ["number"] },
        { frame: stripe(4), roles: ["verificationNumber"] },
      ],
      { tree },
    );
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }] });
  });

  it("ignores scan answers from another tab or for an unknown scan", async () => {
    const other: FrameRef = { ...stripe(3), tabId: 2 };
    const { h, requests } = setup(findCards, [{ frame: other, roles: ["number"] }]);
    await h.handleContent(stripe(4), { type: "cs_card_fields", scan: "f".repeat(32), roles: ["number"] });
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }] });
  });

  it("caps the frames", async () => {
    const reports = Array.from({ length: 20 }, (_, i) => ({ frame: stripe(i + 1), roles: ["number" as const] }));
    const { h, requests } = setup(findCards, reports);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    const fill = requests.find((r) => r.type === "fill_card") as Extract<Request, { type: "fill_card" }>;
    expect(fill.frames).toHaveLength(8);
    // At most 16 reports were kept, so at most 7 extra frames were looked up.
    expect(requests.filter((r) => r.type === "find_cards").length).toBeLessThanOrEqual(1 + 16);
  });

  it("refuses a card the menu never offered, and says so when a card was deleted", async () => {
    const { h } = setup((r) => (r.type === "fill_card" ? new BridgeError("not_found", "x") : findCards(r)));
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: "22222222-2222-4222-8222-222222222222" })).toMatchObject({ ok: false });
    const again = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: again.token, itemId: VISA })).toEqual({ ok: false, message: "This card is no longer in HavenKeys." });
  });

  it("does not fill a card from a login menu", async () => {
    const { h, requests } = setup((r) => (r.type === "find_matches" ? { type: "find_matches", matches: [] } : findCards(r)));
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "login", explicit: true })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA })).toMatchObject({ ok: false });
    expect(requests.some((r) => r.type === "fill_card")).toBe(false);
  });
});

describe("card save prompt", () => {
  const typed = { number: "4000056655665556", expiry: "2030-01", verificationNumber: "123", cardholderName: "Samuel" };

  it("offers a new card in the top frame and saves it on confirm", async () => {
    const { h, requests, sent } = setup((r) => (r.type === "save_card" ? { type: "save_card", itemId: VISA } : findCards(r)));
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    // The number stays in the background until the user confirms.
    expect(JSON.stringify(requests)).not.toContain(typed.number);
    const show = sent.find((s) => s.msg.type === "bg_show_save")!;
    expect(show.frameId).toBe(0);
    const token = (show.msg as { token: string }).token;
    expect(await h.handleInline(1, { type: "save_state", token })).toEqual({
      ok: true,
      value: { site: "shop.com", title: "Visa", card: { brand: "visa", last4: "5556", expiry: "01/30" } },
    });
    expect(await h.handleInline(1, { type: "save_confirm", token, title: "Meu Visa" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_card", url: SHOP, title: "Meu Visa", number: typed.number, expiry: "2030-01", verificationNumber: "123", cardholderName: "Samuel" });
    // Saved once: the prompt is gone.
    expect(await h.handleInline(1, { type: "save_state", token })).toMatchObject({ ok: false });
  });

  it("stays quiet for a saved card, an iframe, http and a failing number", async () => {
    const { h, sent } = setup(findCards);
    await h.handleContent(top, { type: "cs_card_submit", card: { ...typed, number: "4111111111111111", expiry: "2033-04" } }); // same last 4 and expiry as VISA
    await h.handleContent(stripe(3), { type: "cs_card_submit", card: typed });
    await h.handleContent({ ...top, url: "http://shop.com/", origin: "http://shop.com" }, { type: "cs_card_submit", card: typed });
    await h.handleContent(top, { type: "cs_card_submit", card: { ...typed, number: "4000056655665557" } });
    expect(sent.filter((s) => s.msg.type === "bg_show_save")).toEqual([]);
  });

  it("survives the checkout's navigation, and is dropped on dismiss", async () => {
    const { h, requests } = setup(findCards);
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    const next: FrameRef = { ...top, url: "https://shop.com/thanks" };
    const r = (await h.handleContent(next, { type: "cs_ready" })) as { saveToken: string };
    expect(r.saveToken).toEqual(expect.any(String));
    // A subframe of the new page is not offered the prompt.
    expect(await h.handleContent(stripe(3), { type: "cs_ready" })).toMatchObject({ saveToken: null });
    await h.handleInline(1, { type: "save_dismiss", token: r.saveToken });
    expect(await h.handleContent(next, { type: "cs_ready" })).toMatchObject({ saveToken: null });
    expect(requests.some((x) => x.type === "save_card")).toBe(false);
  });

  it("replaces a pending login prompt, and a login submit replaces it", async () => {
    const { h, sent } = setup((r) => (r.type === "check_login" ? { type: "check_login", action: "add", itemId: null } : findCards(r)));
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    const cardToken = (sent.find((s) => s.msg.type === "bg_show_save")!.msg as { token: string }).token;
    await h.handleContent(top, { type: "cs_submit", username: "u", password: "p" });
    expect(sent.some((s) => s.msg.type === "bg_close_save" && (s.msg as { token: string }).token === cardToken)).toBe(true);
    expect(await h.handleInline(1, { type: "save_state", token: cardToken })).toMatchObject({ ok: false });
  });

  it("drops the typed card after two minutes and on lock", async () => {
    const { h } = setup(findCards);
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: expect.any(String) });
    vi.advanceTimersByTime(120_001);
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: null });
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    h.reset();
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: null });
  });
});
