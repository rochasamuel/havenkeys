import { useEffect, useId, useState, type ReactNode } from "react";
import { Icon } from "./Icon";

/** Ask a Disclosure by id to open, e.g. "Make an encrypted backup first" → Export. */
export const OPEN_SECTION_EVENT = "havenkeys:open-section";

export function openSection(id: string) {
  window.dispatchEvent(new CustomEvent<string>(OPEN_SECTION_EVENT, { detail: id }));
}

/**
 * A settings section that opens in place: a row with its title and a one-line
 * summary, and the section underneath. A closed section is not rendered, so
 * any password typed into it does not outlive closing it.
 */
export function Disclosure({ id, title, summary, children }: { id: string; title: string; summary: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const bodyId = useId();

  useEffect(() => {
    const onOpen = (e: Event) => {
      if ((e as CustomEvent<string>).detail === id) setOpen(true);
    };
    window.addEventListener(OPEN_SECTION_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_SECTION_EVENT, onOpen);
  }, [id]);

  return (
    <section className={open ? "disclosure open" : "disclosure"} id={id}>
      <h3 className="disclosure-head">
        <button type="button" aria-expanded={open} aria-controls={bodyId} onClick={() => setOpen((o) => !o)}>
          <span className="disclosure-text">
            <span className="disclosure-title">{title}</span>
            <span className="disclosure-summary">{summary}</span>
          </span>
          <Icon name="chevronRight" size={16} className="disclosure-chevron" />
        </button>
      </h3>
      {open && (
        <div className="disclosure-body" id={bodyId}>
          {children}
        </div>
      )}
    </section>
  );
}
