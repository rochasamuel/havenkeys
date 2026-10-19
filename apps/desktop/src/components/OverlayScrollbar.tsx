import { useCallback, useEffect, useRef, useState, type PointerEvent, type RefObject } from "react";

const HIDE_MS = 900;
const MIN_THUMB = 24;
/** A click on the track pages by this share of the visible height. */
const PAGE = 0.9;

interface Geometry {
  /** The track's place in the target's parent: over the target, at its right. */
  offset: number;
  track: number;
  thumbTop: number;
  thumb: number;
}

/**
 * A scrollbar that floats in `target`'s parent (positioned) instead of taking
 * width from the target (whose native bar must be hidden), as on macOS. It
 * shows while the target or the bar is hovered, or the target scrolls; the
 * thumb drags and a click on the track pages.
 */
export function OverlayScrollbar({ target }: { target: RefObject<HTMLElement | null> }) {
  const [geometry, setGeometry] = useState<Geometry | null>(null);
  const [hovered, setHovered] = useState(false);
  const [scrolling, setScrolling] = useState(false);
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ y: number; scrollTop: number } | null>(null);

  const measure = useCallback(() => {
    const el = target.current;
    if (!el) return;
    const { scrollTop, scrollHeight, clientHeight, offsetTop } = el;
    if (scrollHeight <= clientHeight) return setGeometry(null);
    const thumb = Math.max(MIN_THUMB, (clientHeight * clientHeight) / scrollHeight);
    const thumbTop = (scrollTop / (scrollHeight - clientHeight)) * (clientHeight - thumb);
    setGeometry({ offset: offsetTop, track: clientHeight, thumbTop, thumb });
  }, [target]);

  useEffect(() => {
    const el = target.current;
    if (!el) return;
    let timer: number | undefined;
    const onScroll = () => {
      measure();
      setScrolling(true);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setScrolling(false), HIDE_MS);
    };
    const onEnter = () => {
      measure();
      setHovered(true);
    };
    const onLeave = () => setHovered(false);
    window.addEventListener("resize", measure);
    el.addEventListener("scroll", onScroll, { passive: true });
    el.addEventListener("pointerenter", onEnter);
    el.addEventListener("pointerleave", onLeave);
    return () => {
      window.removeEventListener("resize", measure);
      el.removeEventListener("scroll", onScroll);
      el.removeEventListener("pointerenter", onEnter);
      el.removeEventListener("pointerleave", onLeave);
      window.clearTimeout(timer);
    };
  }, [target, measure]);

  if (!geometry) return null;

  const onThumbDown = (e: PointerEvent<HTMLDivElement>) => {
    const el = target.current;
    if (!el || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { y: e.clientY, scrollTop: el.scrollTop };
    setDragging(true);
  };
  const onThumbMove = (e: PointerEvent<HTMLDivElement>) => {
    const el = target.current;
    if (!el || !drag.current) return;
    const perPixel = (el.scrollHeight - el.clientHeight) / (geometry.track - geometry.thumb);
    el.scrollTop = drag.current.scrollTop + (e.clientY - drag.current.y) * perPixel;
  };
  const onThumbUp = () => {
    drag.current = null;
    setDragging(false);
  };
  const onTrackDown = (e: PointerEvent<HTMLDivElement>) => {
    const el = target.current;
    if (!el || e.button !== 0) return;
    e.preventDefault();
    const y = e.clientY - e.currentTarget.getBoundingClientRect().top;
    const page = el.clientHeight * PAGE;
    el.scrollBy({ top: y < geometry.thumbTop ? -page : page });
  };

  return (
    <div
      className="overlay-track"
      data-visible={hovered || scrolling || dragging}
      data-dragging={dragging}
      style={{ top: geometry.offset, height: geometry.track }}
      onPointerDown={onTrackDown}
      aria-hidden="true"
    >
      <div
        className="overlay-thumb"
        style={{ height: geometry.thumb, transform: `translateY(${geometry.thumbTop}px)` }}
        onPointerDown={onThumbDown}
        onPointerMove={onThumbMove}
        onPointerUp={onThumbUp}
        onPointerCancel={onThumbUp}
      />
    </div>
  );
}
