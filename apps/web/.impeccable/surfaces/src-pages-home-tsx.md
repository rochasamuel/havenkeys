---
version: 1
slug: "src-pages-home-tsx"
primary_target: "src/pages/Home.tsx"
related_targets: ["src/pages/Security.tsx","src/pages/Download.tsx"]
---

# Surface brief: havenkeys.net homepage (+ Security, Download)

Scope: `apps/web` marketing site. Mode: Persuade (Security page leans Read, same world).
Audience: technical self-hosters evaluating whether to trust HavenKeys; today, the author showing it to others.
Action: download the desktop app; secondary, read how the security works.
Proof on hand: real extension screenshots (store set), the threat model's A1–A12 regression attacks, real parameters (Argon2id 128 MiB t=4 p=4, AES-256-GCM, 128-bit Secret Key), real permission list. Desktop app screenshots do not exist yet: labeled placeholders.
Constraints: verbatim audit disclaimer; no invented users, benchmarks or claims; no "unhackable"-class language.

## Direction contract

THESIS: The page follows one login (github.com, sam@example.com) from the moment it is typed to the moment it is filled, so security is a story the visitor watches rather than a spec sheet. Refuses the feature-card grid and the doc-link list the old site shipped.

OWN-WORLD: Deep forest green grounds (#0b110f → #172320) with brass (#c9a45c / #e3c483) as the single fitting; one light paper chapter (Emergency Kit) as a page-scale field change. Source Serif 4 display (optical size, italic for the human voice) over Hanken Grotesk body; JetBrains Mono only for real keys, ciphertext, codes and commands. Hairline green rules, 8–10px radii, framed product windows with soft offset shadows. Shield-keyhole mark.

STORY: Visitor sees the product itself first, understands in four chapters that keys never leave the device and the server holds only ciphertext, sees the browser autofill in real screenshots, reads honestly what is and isn't defended, downloads.

FIRST VIEWPORT: Slim nav. Centered serif headline at ~5.5rem over two lines, one-sentence lede, Download (brass) + "See how it works" side by side. Below, rising into the fold: a large desktop window (placeholder) with the real extension popup screenshot overlapping its right edge.

FORM: Scroll-told product film, candidate 2 of 7 on the ranked list; seed key 47cb7b9b. Signature interaction: a sticky "journey stage" beside four chapters whose state (keys → sealed → stored → filled) changes as each chapter enters, with the plaintext item visibly scrambling into ciphertext on step two. Motion: one orchestrated stage; everything else static.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
