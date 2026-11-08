---
version: 1
slug: "app-landing-ejs"
primary_target: "app/landing.ejs"
related_targets: ["app/settings.ejs","app/frame.ejs"]
---

## Direction contract

THESIS: A server list alongside the garden illustration makes the next session obvious.

OWN-WORLD: Ivory paper, blue ribbon ink, clear Korean system type and one blue bookmark marking the active server.

STORY: Find the saved server, confirm its version and status, then play. Progress uses the same 240px action area as the right-aligned play control, or 220px in compact windows. Preparation owns the selected server and prevents duplicate preparation. Once it finishes, additional games may launch; active games are tracked independently and the button reads Launch another. Unmeasured preparation uses an indeterminate bar. Unavailable status queries leave Play available with only the status label. Launch failures offer retry, the existing report link and redacted diagnostic copy.

FIRST VIEWPORT: A 48px frame and clamp(240px, 20vw, 260px) home sidebar. The artwork fills the remaining width and height above a 112px summary, with no outer padding or card gap. Use cover at center 65%, without distorting the original ratio. Footer actions share an icon/text column; the version shares the report row at its right edge, using 14px muted type. Only the list scrolls; below 760px the list moves above the full-bleed art.

HIERARCHY: Preserve 24px home titles, 20px compact titles and 14px progress text. Give the server title 750 weight and place version/status in a distinct facts row 6px below it. Align the 200px by 56px Play button with the summary right inset. Preserve all existing palette values and the shared frame flex alignment.

SETTINGS: Seven labeled line icons, one selected navigation surface and a divider before About and Updates. Content uses a maximum 840px reading width and 24px section rhythm. The content uses 24px block padding and inline padding capped at 40px. The tab heading has no bottom rule and separates from content with a 20px gap, or 16px in compact windows. Game and launcher option rows use 14px block padding without horizontal dividers. Mod groups and release notes use headings and horizontal rules instead of outer cards. The current account has a selected surface and horizontal actions; Java memory statistics are inline beneath the sliders. Version state and its actions share one wrapping row with 20px padding. Navigation rows start at 42px, or 36px in short windows. Mod rows start at 52px with 8px block padding; server context and account actions use 12px padding, with account rows starting at 88px. The home action summary starts at 100px with 16px by 20px padding, or 12px padding and artwork gap in compact windows.

ASSETS: Keep the existing garden, mascot and bundled fonts. This refinement adds no raster asset or library; navigation symbols are small source-authored SVG geometry.

FORM: User-selected garden guidebook. Code-led Operate surface.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

SIDEBAR REFINEMENT: Align heading, 32px server icons and footer controls on the same inset. Keep 16px labels, 60px rows, 10px icon gaps and padding. Selected names use ribbon blue with a 24px centered bookmark. Footer actions use equal 40px rows and 500 weight. The rail bottom padding and footer top padding are both 12px. No heading icon.
