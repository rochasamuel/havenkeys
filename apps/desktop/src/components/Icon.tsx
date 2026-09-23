// Inline SVG icons (no icon font, no remote assets). One family: 24px grid,
// 1.6 stroke, round caps and joins, drawn to sit with Hanken Grotesk.

const paths = {
  lock: "M7.5 10.5V8a4.5 4.5 0 0 1 9 0v2.5M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1zM12 14.5v2",
  unlock: "M7.5 10.5V8a4.5 4.5 0 0 1 8.7-1.6M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1z",
  search: "M10.5 17.5a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4.5-4.5",
  key: "M14.5 3.5a6 6 0 1 1-5.2 9L4 17.8V20h2.5v-2h2v-2h2l1.4-1.4A6 6 0 0 1 14.5 3.5zM16 8h.01",
  note: "M7 3.5h7l4.5 4.5v11.5a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-15a1 1 0 0 1 1-1zM13.5 3.5V8.5h5M9 12.5h6M9 16h4",
  grid: "M5 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM14 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM5 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5zM14 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5z",
  dice: "M12 3.5 19.5 7.5v9L12 20.5 4.5 16.5v-9L12 3.5zM4.5 7.5 12 11.5l7.5-4M12 11.5v9",
  gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM10.3 3.5h3.4l.5 2.3 1.6.9 2.2-.8 1.7 3-1.8 1.5v1.8l1.8 1.5-1.7 3-2.2-.8-1.6.9-.5 2.3h-3.4l-.5-2.3-1.6-.9-2.2.8-1.7-3 1.8-1.5v-1.8L4.3 8.9l1.7-3 2.2.8 1.6-.9.5-2.3z",
  plus: "M12 5v14M5 12h14",
  copy: "M9 8.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM16 8.5V6a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h2",
  check: "M5 12.5l4.5 4.5L19 7.5",
  eye: "M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  eyeOff: "M4 4l16 16M10.2 5.7A9 9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.9 3.7M6.6 6.9A15.6 15.6 0 0 0 2.5 12S6 18.5 12 18.5a8.8 8.8 0 0 0 4.6-1.3M9.9 9.9a3 3 0 0 0 4.2 4.2",
  edit: "M4.5 19.5h4l10-10a2.1 2.1 0 0 0-3-3l-10 10v3zM14 8l3 3",
  trash: "M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2M6.5 7l.9 12.1a1 1 0 0 0 1 .9h7.2a1 1 0 0 0 1-.9L17.5 7",
  globe: "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM3.5 12h17M12 3.5c2.3 2.3 3.5 5.2 3.5 8.5s-1.2 6.2-3.5 8.5c-2.3-2.3-3.5-5.2-3.5-8.5S9.7 5.8 12 3.5z",
  refresh: "M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4.5H15",
  x: "M6.5 6.5l11 11M17.5 6.5l-11 11",
  shield: "M12 3.5l7 2.8V12c0 4.3-3 7.4-7 8.8-4-1.4-7-4.5-7-8.8V6.3l7-2.8z",
  clock: "M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM12 7.5V12l3 2",
  arrowRight: "M5 12h14M13 6l6 6-6 6",
  chevronDown: "M6.5 9.5 12 15l5.5-5.5",
  cloud: "M7 18.5h10a4 4 0 0 0 .6-8A5.5 5.5 0 0 0 7 9.5a4.5 4.5 0 0 0 0 9z",
  cloudOff: "M4 4l16 16M9 6.4A5.5 5.5 0 0 1 17.6 10.5 4 4 0 0 1 19.8 17M17 18.5H7a4.5 4.5 0 0 1-1.7-8.7",
  laptop: "M5.5 6h13a1 1 0 0 1 1 1v8.5h-15V7a1 1 0 0 1 1-1zM2.5 18h19",
  alert: "M12 4 21 19.5H3L12 4zM12 10v4M12 17h.01",
  download: "M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14",
  printer: "M7 9V4h10v5M7 17H5a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-2M7 14h10v6H7z",
} as const;

export type IconName = keyof typeof paths;

export function Icon({ name, size = 18, className }: { name: IconName; size?: number; className?: string }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={paths[name]} />
    </svg>
  );
}
