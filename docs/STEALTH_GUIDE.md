# Stealth Mode Guide

## Compositor Support

| Compositor | Stealth Level | Notes |
|------------|---------------|-------|
| Hyprland | Full (with rule) | `no_screen_share` hides Shaya from screenshots and screen share |
| Sway | Partial | Hidden from Alt+Tab; visible in `grim` and PipeWire capture |
| KDE Plasma | Partial | Layer-shell overlay works; no compositor-level capture hiding |
| GNOME Wayland | None | No layer-shell; share a specific window as workaround |
| X11 (any DE) | None | No layer-shell; share a specific window as workaround |

---

## Stealth Behavior Matrix

| Environment | Screenshots | Zoom/Meet/OBS (PipeWire) | Alt+Tab |
|-------------|-------------|--------------------------|---------|
| Hyprland + `no_screen_share` | ✅ Hidden | ✅ Hidden | ✅ Hidden |
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

---

## Hyprland Configuration

```ini
layerrule = no_screen_share on, match:namespace shaya-stealth
```

### Verify

```bash
hyprctl layers | grep shaya
```

---

## Fallback: Window Sharing

If stealth is unavailable (Sway, KDE, GNOME, X11), share a **specific app window** in Zoom/Meet/OBS instead of the full screen.

This keeps Shaya out of the shared content as long as the overlay stays on a different surface/workspace.

"The quieter you become, the more you are able to hear" (c) 
