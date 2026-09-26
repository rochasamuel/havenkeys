// Tests assert the English UI. The extension picks its language from the
// browser (src/i18n/index.ts), and Node derives navigator.languages from the
// system locale, so pin it here: a developer with a Portuguese system
// locale would otherwise run the suite in Portuguese.
Object.defineProperty(globalThis.navigator, "languages", { value: ["en-US"], configurable: true });
Object.defineProperty(globalThis.navigator, "language", { value: "en-US", configurable: true });
