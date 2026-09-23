---
version: 1
slug: "src-app-tsx"
primary_target: "src/App.tsx"
related_targets: ["src/views/VaultScreen.tsx","src/views/UnlockScreen.tsx"]
---

# Surface brief: HavenKeys desktop app

Scope: `apps/desktop/src` (Tauri window): welcome, unlock, Emergency Kit, vault (sidebar, list, detail, editor), generator, settings. Mode: Operate.
User: the author, daily. Tasks: unlock, find, reveal/copy, fill TOTP, add/edit, lock. Constraints: no behavior or security change; secrets only on explicit reveal; lock/online state always visible; dark, light and system themes all supported; CSP style-src 'self'.
Brief (user, pinned): same premises as havenkeys.net, professional, smooth like Apple apps, animation where it helps, new icons and fonts matching the website. macOS gets an overlay title bar.

## Direction contract

THESIS: A calm, native-feeling vault in the grammar of Apple's own apps (source-list sidebar, list, detail, inset grouped rows) wearing HavenKeys' forest green, brass and serif titles. Refuses the bordered-card-per-field dashboard look of the old build.

OWN-WORLD: Forest-green grounds from @havenkeys/ui with the website's deeper ground; brass only as the fitting (selection, primary action, focus, live TOTP). Source Serif 4 for titles (unlock headline, pane titles, item title), Hanken Grotesk for UI, JetBrains Mono for secrets. Hairline separators inside 12px inset groups, 8px row radii, soft offset shadows only on floating things. The shield-and-keyhole mark replaces the padlock seal.

STORY: Open → unlock in one field → find with search or sidebar → reveal or copy with an action that confirms itself → lock with Cmd/Ctrl+L or the sidebar footer, which always shows lock and sync state.

FIRST VIEWPORT: Unlock: centered keyhole mark over a faint guilloche rosette, serif "HavenKeys is locked" at ~30px, one-line reason, a single password field with an inline brass arrow button; the Secret Key field slides in when needed. Vault: 232px sidebar (traffic-light inset on macOS), 300px list with serif pane title, detail with 56px avatar and 28px serif title over inset groups.

FORM: Apple three-pane source-list app, brief-pinned ("like Apple apps"); no surface roll was run. Signature interaction: on unlock the keyhole turns and the vault rises in (scale 0.985 → 1, fade), panes cross-fade on selection, copy buttons morph to a check, the TOTP ring drains continuously. Motion grammar: one ease (0.32, 0.72, 0, 1), 180/280/420 ms, everything off under reduced motion.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
