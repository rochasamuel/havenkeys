// Entry point for the MAIN-world script (see page.ts).
import { install } from "./page";

install(window as Window & typeof globalThis);
