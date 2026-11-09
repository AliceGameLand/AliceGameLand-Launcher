# Custom Windows setup

Mode: Operate. Scope: installer/index.html, installer/style.css, installer/renderer.js, installer/main.cjs, installer/uninstall/.
Seed: none (precise requested replacement of the Windows wizard inside the existing brand).

## THESIS
Installing the launcher is the first room of GameLand: a dedicated frameless window using its header mascot, ivory and ribbon blue.

## STORY
Choose the location and user scope, install, then launch or finish. Preserve the NSIS install/uninstall engine, bundled Java and update metadata. Do not modify the installed launcher during development.

Windows removal reuses the same header, typography and progress bar. Show what stays in one line, focus Cancel on entry and show completion after NSIS removes the installation.

## FIRST VIEWPORT
A compact 720x520 window with a custom 48px draggable header and one full-width installation column. The user requested removing the entire garden panel; the installation title, path picker and primary action now fill that space. Keep 20px group spacing and a 200px completion action directly below the installed path. No native wizard navigation or decorative progress claims.

## FORM
Code-led extension of the established identity. User explicitly rejected a reskinned native wizard and requested a completely different window. Existing artwork/font are reused from the repository; asset origins ship in asset-origins.json. Signature: the full-width column transitions between location, real checking progress, indeterminate installation and launch.

## INTERACTION
Actual bytes determine file verification percentage; installation is indeterminate because the backend has no reliable percentage callback. Prevent duplicate installation and closing during writes. Report errors with retry; never stop a running launcher or game automatically. Respect reduced motion.

## FINISH
Inspect 720x520 and 640x480, long paths, keyboard focus, error/retry and complete states in isolated Electron. Keep preferred one-line home server names. Build Windows EXE without publishing. Fresh finish review and documentation required.
