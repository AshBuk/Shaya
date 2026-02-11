# Stealth Mode Guide

## Compositor Support

| Compositor | Stealth Level | Notes |
|------------|---------------|-------|
| Hyprland | Full (with rule) | `no_screen_share` hides ShyQuiet in screenshot/screen-share |
| Sway | Partial | Hidden from Alt+Tab, visible in `grim` and PipeWire capture |
| GNOME Wayland / X11 | None | No layer-shell stealth, use window sharing |

---

## Capture Behavior Matrix

| Environment | `grim` / wlr-screencopy | Zoom/Meet/OBS (PipeWire) | Alt+Tab |
|-------------|--------------------------|---------------------------|---------|
| Hyprland + `no_screen_share` | ✅ Hidden | ✅ Hidden | ✅ Hidden |
| Sway | ❌ Visible | ❌ Visible | ✅ Hidden |
| GNOME Wayland / X11 | ❌ Visible | ❌ Visible | ❌ Visible |

## Hyprland Configuration

```ini
layerrule = no_screen_share on, match:namespace shyquiet-stealth
```

### Verify

```bash
hyprctl layers | grep shyquiet
```

---

## Fallback: Window Sharing

If stealth is unavailable (Sway without compositor rule, GNOME, X11), share a specific app window instead of the full screen.

This keeps ShyQuiet out of the shared content as long as the overlay stays on a different surface/workspace.
