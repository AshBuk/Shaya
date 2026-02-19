# Stealth Mode Guide

## Compositor Support

| Compositor | Stealth Level | Notes |
|------------|---------------|-------|
| Hyprland | Partial (with rule) | `no_screen_share` replaces content with a black rectangle in screenshots and screen share |
| Sway | Partial | Hidden from Alt+Tab; visible in `grim` and PipeWire capture |
| KDE Plasma | Partial | Layer-shell overlay works; no compositor-level capture hiding |
| GNOME Wayland | None | No layer-shell; share a specific window as workaround |
| X11 (any DE) | None | No layer-shell; share a specific window as workaround |

---

## Stealth Behavior Matrix

| Environment | Screenshots | Zoom/Meet/OBS (PipeWire) | Alt+Tab |
|-------------|-------------|--------------------------|---------|
| Hyprland + `no_screen_share` | ⬛ Black rectangle | ⬛ Black rectangle | ✅ Hidden |
| Sway | ❌ Visible | ❌ Visible | ✅ Hidden |
| KDE Plasma Wayland | ❌ Visible | ❌ Visible | ✅ Hidden |
| GNOME Wayland | ❌ Visible | ❌ Visible | ❌ Visible |
| X11 (any DE) | ❌ Visible | ❌ Visible | ❌ Visible |

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

```ini
layerrule = no_screen_share on, match:namespace shaya-stealth
```

---

## Always-on-Top Support

Shaya is designed to stay on top of all windows as a desktop overlay.

### Layer-shell supported (always-on-top works automatically)

| DE / Compositor | Protocol | Notes |
|---|---|---|
| Hyprland | wlr-layer-shell | Full support, best experience |
| Sway | wlr-layer-shell | wlroots-based, works out of the box |
| KDE Plasma 6 (Wayland) | wlr-layer-shell | Overlay layer supported |
| COSMIC | wlr-layer-shell | smithay-based, layer-shell supported |
| river, labwc, wayfire | wlr-layer-shell | wlroots-based |

### Layer-shell not supported (falls back to normal window)

| DE / Compositor | Reason |
|---|---|
| GNOME (Mutter) | Does not implement wlr-layer-shell |
| Cinnamon | X11-only (no Wayland support yet) |
| MATE | X11-only |
| Xfce | Wayland support in progress, no layer-shell yet |
| Budgie | Migrating to custom Wayland stack |
| X11 (any DE) | Layer-shell is a Wayland-only protocol |

### GNOME Workaround

GNOME does not support always-on-top programmatically. To keep Shaya on top:

1. Right-click the Shaya title bar
2. Enable **"Always on Top"**
3. Enable **"Always on Visible Workspace"**

This must be done manually each time the app starts, or use a GNOME extension (e.g. `Window Is Ready - Remover` or a custom rule) to automate it.

---

## Fallback: Window Sharing

If stealth is unavailable (Sway, KDE, GNOME, X11), share a **specific app window** in Zoom/Meet/OBS instead of the full screen.

This keeps Shaya out of the shared content as long as the overlay stays on a different surface/workspace.

"The quieter you become, the more you are able to hear" (c) 
