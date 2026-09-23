import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/context";
import { SCENES } from "./Scenes";

/* On narrow screens each chapter carries its own scene; it plays when it is
   itself on screen, not when the (hidden) sticky stage says so. */
function InlineScene({ index }: { index: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [seen, setSeen] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => {
      if (e?.isIntersecting) setSeen(true);
    }, { threshold: 0.4 });
    io.observe(el);
    return () => io.disconnect();
  }, []);
  const Scene = SCENES[index]!;
  return (
    <div className="stage stage--inline" ref={ref}>
      <Scene active={seen} />
    </div>
  );
}

export function Journey() {
  const { t } = useI18n();
  const CHAPTERS = t.journey.chapters;
  const [active, setActive] = useState(0);
  const chapters = useRef<Array<HTMLElement | null>>([]);

  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) setActive(Number((e.target as HTMLElement).dataset.step));
        }
      },
      { rootMargin: "-45% 0px -45% 0px" },
    );
    for (const el of chapters.current) if (el) io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <div className="journey">
      <div className="journey__chapters">
        {CHAPTERS.map((c, i) => {
          return (
            <article
              key={i}
              className={`chapter ${active === i ? "is-active" : ""}`}
              data-step={i}
              ref={(el) => {
                chapters.current[i] = el;
              }}
            >
              <h3>{c.title}</h3>
              {c.body.map((para, n) => (
                <p key={n}>{para}</p>
              ))}
              <div className="chapter__scene">
                <InlineScene index={i} />
              </div>
            </article>
          );
        })}
      </div>
      <div className="journey__stage" aria-hidden="true">
        <div className="stage">
          <ol className="stage__rail">
            {CHAPTERS.map((c, i) => (
              <li key={i} className={i === active ? "is-active" : i < active ? "is-done" : ""}>
                {c.label}
              </li>
            ))}
          </ol>
          <div className="stage__scenes">
            {SCENES.map((Scene, i) => (
              <div key={i} className={`stage__slot ${i === active ? "is-active" : ""}`}>
                <Scene active={i === active} />
              </div>
            ))}
          </div>
          <p className="stage__foot">{t.journey.stageFoot}</p>
        </div>
      </div>
    </div>
  );
}
