/* One stroke family for the whole site: 20px grid, 1.6 stroke, round caps. */
const PATHS = {
  arrowRight: "M4 10h12M11 5l5 5-5 5",
  arrowDown: "M10 4v12M5 11l5 5 5-5",
  download: "M10 3v10M5.5 8.5 10 13l4.5-4.5M4 16.5h12",
  check: "M4.5 10.5l3.5 3.5 7.5-8",
  cross: "M5.5 5.5l9 9M14.5 5.5l-9 9",
  lock: "M6 9V6.8a4 4 0 0 1 8 0V9M4.5 9h11v8h-11z",
  key: "M7 13.5a3.5 3.5 0 1 1 3.4-4.5H17v2.5h-2v2h-2.5v-2h-2.1A3.5 3.5 0 0 1 7 13.5ZM6.2 10h.01",
  server: "M3.5 4h13v5h-13zM3.5 11h13v5h-13zM6.5 6.5h.01M6.5 13.5h.01",
  laptop: "M4.5 5h11v8h-11zM2.5 15.5h15",
  monitor: "M3 4h14v9.5H3zM7.5 16.5h5M10 13.5v3",
  external: "M8 4.5H4.5v11h11V12M11 4h5v5M16 4l-7 7",
  github:
    "M10 2.5a7.5 7.5 0 0 0-2.4 14.6c.4.1.5-.2.5-.4v-1.4c-2.1.5-2.5-.9-2.5-.9-.3-.9-.8-1.1-.8-1.1-.7-.5.1-.5.1-.5.8.1 1.2.8 1.2.8.7 1.2 1.8.8 2.2.6.1-.5.3-.8.5-1-1.7-.2-3.4-.8-3.4-3.7 0-.8.3-1.5.8-2-.1-.2-.3-1 .1-2 0 0 .6-.2 2.1.8a7 7 0 0 1 3.8 0c1.4-1 2.1-.8 2.1-.8.4 1 .1 1.8.1 2 .5.5.8 1.2.8 2 0 2.9-1.8 3.5-3.4 3.7.3.2.5.7.5 1.4v2.1c0 .2.1.5.5.4A7.5 7.5 0 0 0 10 2.5Z",
  image: "M3.5 4.5h13v11h-13zM3.5 13l4-4 3.5 3.5 2-2 3.5 3.5M12.5 8h.01",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 18, className }: { name: IconName; size?: number; className?: string }) {
  const filled = name === "github";
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 20 20"
      aria-hidden="true"
      focusable="false"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
