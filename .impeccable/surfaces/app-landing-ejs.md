---
version: 1
slug: "app-landing-ejs"
primary_target: "app/landing.ejs"
related_targets: ["app/settings.ejs","app/frame.ejs"]
---

## Direction contract

THESIS: A server list alongside the garden illustration makes the next session obvious.

OWN-WORLD: Ivory paper, blue ribbon ink, clear Korean system type and one blue bookmark marking the active server.

STORY: Find the saved server, confirm its version and status, then play. Progress uses the same 124px space as the play control. Preparation owns the selected server; an active client prevents another launch.

FIRST VIEWPORT: A 48px frame, a shared clamp(220px, 21vw, 268px) sidebar and a full garden illustration up to 800px wide. Server rows use 36px icons, 16px labels, 12px gaps and a 60px minimum height; long names wrap. Only the list scrolls and footer tools stay fixed. Below 760px, a 68px horizontal list uses 184px by 56px rows, two-line names and 26px icons.

HIERARCHY: Preserve 24px home titles, 20px compact titles and 14px progress text. Play and progress share a 124px slot. Preserve all existing palette values and the shared frame flex alignment.

SETTINGS: Seven labeled line icons, one selected navigation surface and a divider before About and Updates. Content uses a maximum 840px reading width and 24px section rhythm. The content uses 24px block padding and inline padding capped at 40px. The tab heading has no bottom rule and separates from content with a 20px gap, or 16px in compact windows. Game and launcher option rows use 14px block padding without horizontal dividers. Mod groups and release notes use headings and horizontal rules instead of outer cards. The current account has a selected surface and horizontal actions; Java memory statistics are inline beneath the sliders. Version state and its actions share one wrapping row with 20px padding. Navigation rows start at 42px, or 36px in short windows. Mod rows start at 52px with 8px block padding; server context and account actions use 12px padding, with account rows starting at 88px. The home action summary starts at 100px with 16px by 20px padding, or 12px padding and artwork gap in compact windows.

ASSETS: Keep the existing garden, mascot and bundled fonts. This refinement adds no raster asset or library; navigation symbols are small source-authored SVG geometry.

FORM: User-selected garden guidebook. Code-led Operate surface.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
