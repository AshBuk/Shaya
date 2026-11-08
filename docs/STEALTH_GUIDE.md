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

## Capture Exclusion on Linux

macOS and Windows provide system-level APIs to exclude a window from screen capture:

- **Windows**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` (Windows 10 2004+) hides the window from most capture methods, but not an absolute guarantee for all apps and capture modes.
- **macOS**: `NSWindow.sharingType = .none` / `kCGWindowSharingNone` works for some legacy capture paths, but is unreliable with modern capture stacks (e.g. ScreenCaptureKit). Not a guaranteed solution.

**Linux has no equivalent.** Wayland/X11 does not provide a standard protocol to exclude a surface from capture. There are compositor-specific mechanisms and proposals (e.g. `ext-surface-capture-control`), but nothing universally supported yet. On Linux, Shaya relies on the following:

| Area | Shaya on Linux |
|---|---|
| Toolkit | Native GTK4, single binary |
| Always on top | Automatic through the `wlr-layer-shell` Overlay layer |
| Alt+Tab | Hidden on `wlr-layer-shell` compositors |
| D-Bus | No bus name: `NON_UNIQUE` flag, with a file lock for the single instance |
| Screen capture | Blacked out on Hyprland with the layer rule, visible elsewhere 