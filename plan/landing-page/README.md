# Landing page motion redesign

[CHOSEN] Static HTML with inline hero CSS and a small copy/platform entry module. GSAP, ScrollTrigger, SplitText, DrawSVG, ScrambleText, and Lenis load after the first frame; OGL shaders and particles load separately. React and Sonner load lazily for feedback. Nunito titles and Liberation Sans body text are self-hosted subsets.

- Hero: larger 1337 → logo particle morph with continuous visible playback, quick letter flips between Leeting and 1337ing, fast pointer response, and no scanning sweep or promo pill. Copy row and logo-chevron scroll cue remain primary.
- Story: browser frame-cadence meter and flow video; deterministic interactive debugger replay (Container With Most Water) and real recording; AI agents and Assist actions; Competitive Companion and providers; idiomatic Rust and GPUI; gallery; final copy with keyboard shortcut.
- Media: real app footage captured in omabox by the delegated `pnpm ops media:*` commands (codex-3). Videos lazy-attach near the viewport, play only while visible, 120 fps variant on high-refresh screens, posters for reduced motion/Save-Data.
- Tailwind and the shadcn Kbd component are removed; plain CSS.

Verification: site tests and production build pass; serial Rust workspace check and five installer regressions pass. Local deployment installs and verifies leet/1337. Source stills, 120 fps app masters, optimized web assets, and private desktop/mobile page review captures remain tracked. Public Pages deployment is a separate publication step.
