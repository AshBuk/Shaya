# Stealth Mode Guide

## Compositor Support

| Compositor | Stealth Level | Notes |
|------------|---------------|-------|
| Hyprland 0.53+ | Partial | Content hidden (black rect), outline visible |
| Sway | Minimal | Alt+Tab hiding only |
| GNOME/X11 | None | Use window sharing |

> Full invisibility is not supported by any Wayland compositor.

---

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

Share a specific window instead of entire screen — ShyQuiet won't be captured.
