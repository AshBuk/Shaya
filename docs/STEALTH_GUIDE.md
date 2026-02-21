# Stealth Mode Guide

## Compositor Support

| Environment | Stealth | Always on Top | Screenshots | Screen Sharing | Alt+Tab |
|---|---|---|---|---|---|
| Hyprland (with rule) | High | Auto | Blacked out | Blacked out | Hidden |
| Sway | Medium | Auto | Visible | Visible | Hidden |
| KDE Plasma (Wayland) | Medium | Auto | Visible | Visible | Hidden |
| COSMIC | Medium | Auto | Visible | Visible | Hidden |
| river, labwc, wayfire | Medium | Auto | Visible | Visible | Hidden |
| GNOME (Wayland) | Low | Manual | Visible | Visible | Visible |
| Cinnamon, MATE, Xfce, Budgie | Low | Manual | Visible | Visible | Visible |
| X11 (any DE) | Low | Manual | Visible | Visible | Visible |

Always-on-top works automatically on compositors with `wlr-layer-shell` support (Hyprland, Sway, KDE Plasma 6, COSMIC, river, labwc, wayfire). On GNOME and X11-based desktops, it must be enabled manually:

1. Right-click the Shaya title bar
2. Enable **"Always on Top"**
3. Enable **"Always on Visible Workspace"**

This must be done manually each time the app starts, or automated via DE-specific tools (GNOME extensions, KWin window rules, devilspie2, wmctrl, etc.).

---

## Screenshot Tools Matrix

| DE | Display | Area Capture | Full Screenshot |
|----|---------|--------------|-----------------|
| Hyprland | Wayland | slurp + grim (sync) | grim (sync) |
| Sway | Wayland | slurp + grim (sync) | grim (sync) |
| GNOME | Wayland | XDG Portal (async) | XDG Portal (async) |
| KDE Plasma | Wayland | spectacle -r (sync) | spectacle (sync) |
| GNOME | X11 | gnome-screenshot -a | gnome-screenshot |
| KDE | X11 | spectacle -r | spectacle |
| Other | X11 | gnome-screenshot / spectacle | gnome-screenshot / scrot / spectacle |

> **Note:** On GNOME Wayland both buttons (area and full screenshot) open the native GNOME
> screenshot dialog via the XDG Desktop Portal (`interactive` mode). The user picks
> full screen, window, or area within the portal UI.

**Hyprland Configuration**

Add this rule to black out Shaya's content in screenshots and screen sharing:

```ini
layerrule = no_screen_share on, match:namespace shaya-stealth
```

---

## Fallback: Window Sharing

If stealth is unavailable on your DE/Compositor, share a **specific app window** in Zoom/Meet/OBS instead of the full screen.

This keeps Shaya out of the shared content as long as the overlay stays on a different surface/workspace.

---

## How Shaya Compares to Other Overlays

macOS and Windows provide system-level APIs to exclude a window from screen capture:

- **Windows**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (Windows 10 2004+) — hides the window from most capture methods, but not an absolute guarantee for all apps and capture modes.
- **macOS**: `NSWindow.sharingType = .none` / `kCGWindowSharingNone` — works for some legacy capture paths, but unreliable with modern capture stacks (e.g. ScreenCaptureKit). Not a guaranteed solution.

**Linux has no equivalent.** Wayland/X11 does not provide a standard protocol to exclude a surface from capture. There are compositor-specific mechanisms and emerging initiatives (e.g. `ext-surface-capture-control` in Wayland staging), but nothing universally supported yet. Shaya uses every mechanism available on Linux:

| | Shaya | Cluely (Electron) | Pluely (Tauri + React) |
|---|---|---|---|
| Toolkit | Native GTK4, single binary | Chromium (Electron), large footprint | WebView (Tauri), smaller than Electron |
| Process visibility | Looks like a regular GTK app | Chromium processes visible in `ps` / `htop` | Single process, less conspicuous |
| Always on top (Linux) | Auto via `wlr-layer-shell` Overlay layer | `alwaysOnTop` flag (unreliable on Wayland) | `alwaysOnTop` flag (unreliable on Wayland) |
| Alt+Tab (Linux) | Hidden on wlroots / KDE | Visible | Visible |
| D-Bus registration | None (`NON_UNIQUE` flag) | Registered, visible in app lists | Registered, visible in app lists |
| Screen capture (Linux) | Blacked out on Hyprland | Fully visible | No-op (`content_protected` unsupported in tao on Linux) |
| Screen capture (macOS) | N/A | Partially hidden (unreliable with ScreenCaptureKit) | Partially hidden (unreliable with ScreenCaptureKit) |
| Screen capture (Windows) | N/A | Mostly hidden (not guaranteed for all capture modes) | Mostly hidden (not guaranteed for all capture modes) |

> **Note:** Pluely uses Tauri's `content_protected` API, which maps to `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on Windows and `NSWindowSharingType::None` on macOS via [tao](https://github.com/tauri-apps/tao). On Linux, this API is a no-op — content protection is unsupported.

**On Linux, Shaya is the maximum stealth-capable option available.** Other overlay tools rely on platform APIs (`CGWindowSharingType`, `SetWindowDisplayAffinity`) that have no Linux equivalent and provide no guarantees even on their native platforms. Shaya is one that leverages Linux-specific mechanisms for actual stealth on Linux.

"The quieter you become, the more you are able to hear" (c)
