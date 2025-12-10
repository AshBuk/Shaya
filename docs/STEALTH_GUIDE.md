# Stealth Mode Guide

## TL;DR — How to Get Full Stealth

| Compositor | Stealth Level | Notes |
|------------|---------------|-------|
| **Hyprland** | Full | `layerrule = noscreenshare, shyquiet-stealth` |
| **Sway** | Partial | Hidden from Alt+Tab only |
| **GNOME/X11** | None | Share specific window as workaround |

**Hyprland is the only compositor that supports true stealth mode.**

---

## Hyprland Setup (Recommended)

Add this line to `~/.config/hypr/hyprland.conf`:

```ini
layerrule = noscreenshare, shyquiet-stealth
```

**That's it.** ShyQuiet will be invisible in:
- ✅ `grim` screenshots
- ✅ `wl-mirror`
- ✅ PipeWire screen share (Zoom, Meet, Teams)
- ✅ OBS screen capture
- ✅ All screencopy-based tools

### Verify It Works

```bash
# Start ShyQuiet
./target/release/shyquiet &

# Take screenshot
grim /tmp/test.png && xdg-open /tmp/test.png
# ShyQuiet should NOT appear
```

---

## How It Works

ShyQuiet uses `wlr-layer-shell` protocol with namespace `shyquiet-stealth`.

Hyprland's `layerrule = noscreenshare on, <namespace>` tells the compositor to **exclude** that layer from all screencopy operations.

```
Hyprland rendering:
┌─────────────────────────────────────────────┐
│           Output Framebuffer                │
│  ┌───────────────────────────────────────┐  │
│  │  All layers...                        │  │
│  │  + ShyQuiet (Layer::Overlay)          │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘

Hyprland screencopy with noscreenshare rule:
┌─────────────────────────────────────────────┐
│        Screencopy Buffer                    │
│  ┌───────────────────────────────────────┐  │
│  │  All layers...                        │  │
│  │  (ShyQuiet EXCLUDED)                  │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

---

## Why Sway Doesn't Work

Sway's `wlr-screencopy` captures the **composited framebuffer** after all layers are merged. There's no filtering mechanism.

| Feature | Sway | Hyprland |
|---------|------|----------|
| Layer-shell | ✅ | ✅ |
| Alt+Tab hiding | ✅ | ✅ |
| grim hiding | ❌ | ✅ (with layerrule) |
| PipeWire hiding | ❌ | ✅ (with layerrule) |

**If you need full stealth, use Hyprland.**

---

## Fallback: Share Specific Window

If you can't use Hyprland, share a **specific window** instead of entire screen:

**Zoom:**
1. Click "Share Screen"
2. Select **"Window"** tab
3. Choose browser/IDE window
4. ShyQuiet overlay not captured

**Google Meet:**
1. Click "Present now"
2. Choose **"A window"**
3. Select target window

**Teams:**
1. Click "Share"
2. Choose **"Window"**

---

## Additional Hyprland Rules (Optional)

```ini
# Required: Hide from screen share
layerrule = noscreenshare, shyquiet-stealth

# Optional: Add blur behind ShyQuiet
layerrule = blur, shyquiet-stealth
layerrule = ignorealpha 0.3, shyquiet-stealth

# Optional: Disable animations for instant appear/disappear
layerrule = noanim, shyquiet-stealth
```

---

## Troubleshooting

**ShyQuiet still visible in screenshots:**
```bash
# Check namespace is set correctly
hyprctl layers | grep shyquiet
# Should show: shyquiet-stealth
```

**Rule not working:**
```bash
# Verify Hyprland version (need 0.40+)
hyprctl version
```

**No layer-shell on GNOME:**
- GNOME doesn't support `wlr-layer-shell`
- ShyQuiet runs as normal window
- Use "Share Window" for pseudo-stealth

---

## Summary

| Scenario | Solution |
|----------|----------|
| Using Hyprland | Add `layerrule = noscreenshare, shyquiet-stealth` |
| Using Sway | Share window, not screen |
| Using GNOME | Share window, not screen |
| Need full stealth + Sway | Switch to Hyprland |
