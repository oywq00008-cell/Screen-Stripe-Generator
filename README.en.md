# Screen Stripe Generator

Fake a patch of "broken screen" stripes on your display — for demo and entertainment only.

**English** ｜ [简体中文](README.md)

---

## What it is

A tiny Windows utility: draw a rough loop anywhere on screen, and it fills that area with
random colored hairlines that span the whole screen — looking like dead columns, colored bars
and flickering artifacts on a failing LCD. The gaps are **truly transparent**: your desktop
stays visible and fully clickable.

It is written from scratch in **Rust + Win32 with zero third-party dependencies**
(the random number generator is hand-written too), and builds into
**a single ~350 KB executable that needs no runtime at all**.

<!-- Preview: uncomment after putting images into docs/
| Main window | Vertical stripes |
| --- | --- |
| ![Main window](docs/main-window.png) | ![Vertical stripes](docs/vertical-stripes.png) |
-->

## Download

**[Get the latest release on GitHub Releases](https://github.com/oywq00008-cell/Screen-Stripe-Generator/releases/latest)**

Portable single file — **no installation required**, just run it. Current version **v0.1**
(353.5 KB, x64). Every release lists the SHA256 of the executable so you can verify integrity.
To build it yourself, see "Building from source" below.

## Features

| Feature | Description |
| --- | --- |
| **Free-form region** | Hold the left button and draw a loop to mark an area; multiple regions supported; right-click / Enter / Esc to finish |
| **Two orientations** | Vertical uses the region's left-right range and spans the full screen height; horizontal uses the top-bottom range and spans the full width; switching redraws instantly |
| **Three color modes** | `Multi` (red / green / blue / black / white, equal probability), `Black` only, `White` only |
| **Random flicker** | When enabled, a few lines per region blink independently (bright 1–5 s, gone 0.1–3 s); the rest stay lit, and a region never goes completely empty |
| **Boss key** | Set a hotkey once; pressing it hides / restores the control panel. **The stripes stay visible**, unaffected |
| **Anti-capture** | When enabled, the stripes cannot be picked up by screenshots or screen recording (off by default, so they are capturable) |
| **Click-through** | The whole overlay is transparent to the mouse — your desktop stays fully usable |
| **Startup disclaimer** | You must explicitly accept it to reach the main window; declining exits immediately, with no skip switch |

## Requirements

- **Windows 10 / 11 (x64)**, verified on Windows 11
- The anti-capture option requires **Windows 10 version 2004 (build 19041) or newer**;
  on older systems it degrades to showing a black block in captures
- No .NET / VC++ runtime needed, and no administrator rights

## Usage

1. Run `screen_stripe.exe`, read the disclaimer and click **Agree & continue**
2. Click **Start selecting region** → the screen dims for selection → hold the left button and
   draw a loop (multiple allowed) → right-click / Enter / Esc to return
3. Click **Multi / Black / White** to generate stripes in that color
4. Toggle **Orientation**, **Random flicker** and **Anti-capture** as needed.
   **Close stripes** only hides the display; your marked regions are remembered
5. Boss key: click the **Boss key** row → press the combination you want
   (must include Ctrl / Alt / Shift / Win) → press it later to hide / restore the panel
6. Press **Esc** anywhere to quit safely and clean up every overlay

## Building from source

Requires Rust (`stable-x86_64-pc-windows-msvc`) plus the MSVC toolchain and Windows SDK.

```bash
cd rust
cargo build --release --target x86_64-pc-windows-msvc
```

For a dependency-free single file (statically linked CRT — runs on any Windows):

```bash
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --target x86_64-pc-windows-msvc
```

Or just double-click `rust/build.bat`, which builds with the static CRT and copies the result
into `rust/dist/`.

## Implementation notes

- **Zero dependencies**: no crates at all — every Win32 call is a hand-written `extern "system"`
  declaration, so the build needs no network access
- **Single file**: statically linked CRT, ~350 KB, no external DLLs
- **Overlay**: a borderless full-screen topmost window using `LWA_COLORKEY` for the visual
  "punch-through", plus `WS_EX_TRANSPARENT` to make the whole layer click-through.
  Note that **a color key only affects rendering, not hit-testing** — the two must be handled
  separately, otherwise a full-screen window swallows every click on the desktop
- **Anti-capture**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`
- **Boss key**: `RegisterHotKey`, with `WM_HOTKEY` received directly by the main message loop
- **UI**: both the main window and the dialog are fully custom-drawn
  (GDI + double buffering + rectangle hit-testing), with no widget toolkit

## Known limitations

- **It cannot cover the Start menu.** The Start menu, search panel, Task View, Alt+Tab and the
  notification center are drawn directly by the Windows compositor above all application
  windows — no ordinary app can render above them (a system design decision, not a defect here).
  The overlay *does* cover the desktop, all normal windows, other topmost windows and the taskbar.
- **Anti-capture only defeats software capture.** Photographing the screen, HDMI capture cards
  and hypervisor-level capture cannot be prevented.
- **Windows only for now** (x64): the transparent layer relies on Windows-specific color-key
  and display affinity APIs. **Ports to macOS and Linux may follow later** — window transparency
  works completely differently on those systems (a window transparency attribute on macOS,
  the X11 SHAPE extension on Linux), so each needs its own implementation.

## Disclaimer

- This is **free and open-source software under the MIT License**. It contains and provides no
  paid content or paid features of any kind.
- It is intended **solely for demonstrating screen effects and for entertainment**.
  Do not use it for any illegal or improper purpose, including but not limited to:
  faking hardware faults to obtain repairs, warranty service or insurance payouts;
  disrupting other people's work or study or causing them losses; or using it in any situation
  that requires authenticity (such as repair claims, evidence or appraisal).
- As stated in the MIT License, the software is provided **"as is", without warranty of any
  kind, express or implied**. **If you use this software for illegal or improper purposes, all
  legal responsibility and consequences are yours alone; the authors and contributors accept no
  liability whatsoever.**

## License

Released under the **MIT License**.
