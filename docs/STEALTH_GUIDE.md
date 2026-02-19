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

This must be done manually each time the app starts, or use a GNOME extension to automate it.

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

## Fallback: Window Sharing

If stealth is unavailable (Sway, KDE, GNOME, X11), share a **specific app window** in Zoom/Meet/OBS instead of the full screen.

This keeps Shaya out of the shared content as long as the overlay stays on a different surface/workspace.

"The quieter you become, the more you are able to hear" (c)
