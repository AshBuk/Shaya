# Overlay and Screen Sharing

Shaya runs as an overlay above your other windows. Whether people see it when you share your screen depends on the compositor. This guide covers how the overlay behaves and how to keep your chat and notes off a shared screen.

## Toggle shortcut

Shaya has no global shortcut of its own. Running `shaya` again shows or hides the window, so bind a key to it:

```
-- Hyprland, hyprland.lua
hl.bind("SUPER + A", hl.dsp.exec_cmd("shaya"))
# Sway
bindsym $mod+a exec shaya
```

On KDE and GNOME, add a custom shortcut with the command `shaya` in the keyboard settings. Ctrl+Q quits.

The AppImage answers to the same command once it is on your `PATH` as `shaya`, see [Install](../README.md#install). Otherwise bind the key to the full path of the file.

## What the overlay does

| Area | Behavior |
|---|---|
| Always on top | Automatic through the `wlr-layer-shell` Overlay layer |
| Alt+Tab | Hidden on `wlr-layer-shell` compositors |
| D-Bus | No bus name: `NON_UNIQUE` flag, with a file lock for the single instance |
| Screen capture | Depends on the compositor, see below |

Compositors with `wlr-layer-shell` include Hyprland, Sway, KDE Plasma 6, COSMIC, river, labwc and wayfire.

## GNOME and X11

These desktops show Shaya as a normal window, so it appears in Alt+Tab. To keep it on top:

1. Right-click the Shaya title bar
2. Enable **"Always on Top"**
3. Enable **"Always on Visible Workspace"**

Repeat this each time the app starts, or automate it with desktop tools such as GNOME extensions, KWin window rules, devilspie2 or wmctrl.

## Keeping Shaya off a shared screen

Linux has no standard protocol to exclude a window from capture. Assume the overlay appears when you share the full screen, unless your compositor has a rule for it.

**Any desktop:** share a single app window in Zoom, Meet or OBS instead of the full screen. This keeps Shaya out of the shared content as long as it stays on a different surface or workspace.

**Hyprland:** a layer rule blacks out Shaya in screenshots and screen sharing. Add this to `hyprland.lua` (Hyprland 0.55 or later):

```lua
hl.layer_rule({ match = { namespace = "shaya-stealth" }, no_screen_share = true })
```

## Recording other people

Tell the participants before you record a meeting or capture system audio, see [Recording Other People](../README.md#recording-other-people).
