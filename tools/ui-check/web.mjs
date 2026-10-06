// Website pages for ui-check: every page in both languages, at a phone and a
// desktop width, full page.
export const webPages = ["/", "/download", "/security", "/self-host", "/developers", "/privacy", "/terms", "/delete-account"];
export const webSizes = [
  { name: "phone", width: 390, height: 844 },
  { name: "desktop", width: 1280, height: 800 },
];
export function webPath(page, locale) {
  if (locale === "en") return page;
  return page === "/" ? "/pt-br" : `/pt-br${page}`;
}
