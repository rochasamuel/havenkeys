// "Change on website" (spec 2026-11-14-change-password-url): Rust picks the
// page from the login's saved websites and opens it in the browser. The
// renderer names the login only; it never sees or passes an address.

import { useState } from "react";
import { api } from "../lib/api";
import type { ItemOverview } from "../lib/types";
import { Icon } from "./Icon";
import { useToast } from "./Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/** A login with an http(s) website: the only items Rust has a page for. */
export function canChangeOnWebsite(item: ItemOverview): boolean {
  return item.itemType === "login" && item.urls.some((u) => /^https?:\/\//i.test(u.url));
}

export function ChangePasswordButton({ item, disabled }: { item: ItemOverview; disabled?: boolean }) {
  const { t } = useI18n();
  const toast = useToast();
  // Rust may ask the site for its well-known URL first (up to a few
  // seconds): one click at a time.
  const [busy, setBusy] = useState(false);
  if (!canChangeOnWebsite(item)) return null;

  async function open() {
    setBusy(true);
    try {
      const source = await api.openChangePassword(item.id);
      if (source === "website") toast(t.detail.changePageNotFound);
    } catch (e) {
      toast(errorMessage(e, t, t.detail.openWebsiteFailed), "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      className="btn btn-small"
      title={t.detail.changeOnWebsiteHint}
      disabled={disabled || busy}
      aria-busy={busy}
      onClick={() => void open()}
    >
      <Icon name="key" size={15} /> {t.detail.changeOnWebsite}
    </button>
  );
}
