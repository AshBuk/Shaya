# Stealth Mode Guide

## How Stealth Works

ShyQuiet uses `Layer::Overlay` via `wlr-layer-shell` protocol.

| Capture Method | Stealth | Why |
|----------------|---------|-----|
| `grim` (wlr-screencopy) | ✅ Hidden | Protocol excludes Overlay layer |
| `swaymsg -t get_tree` | ✅ Hidden | Layer-shell surfaces not in tree |
| Zoom/Meet/Teams (PipeWire) | ❌ **Visible** | PipeWire gets full framebuffer |
| OBS PipeWire capture | ❌ **Visible** | Same as above |

**Bottom line:** Stealth works with `grim`, but NOT with Zoom/Meet when sharing entire screen.

---

## Solutions

### Option 1: Share Specific Window (Simple)

In Zoom/Meet, choose **"Share Window"** instead of **"Share Screen"**.
Select your browser or IDE window — ShyQuiet overlay won't be captured.

### Option 2: Virtual Monitor (Full Screen Share)

Use `wl-mirror` to create a "clean" virtual monitor without overlay.

---

## Virtual Monitor Setup (Sway)

### 1. Install wl-mirror

```bash
# Fedora
sudo dnf install wl-mirror

# Arch
sudo pacman -S wl-mirror

# Build from source
git clone https://github.com/Ferdi265/wl-mirror.git
cd wl-mirror && meson build && ninja -C build
sudo ninja -C build install
```

### 2. Create Script

Save as `~/.config/sway/scripts/stealth-mirror.sh`:

```bash
#!/bin/bash
# Stealth Mirror - screen share without ShyQuiet overlay
# Usage: stealth-mirror.sh [start|stop|toggle]

HEADLESS="HEADLESS-1"
PID_FILE="/tmp/stealth-mirror.pid"

start() {
    [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null && {
        notify-send "Stealth Mirror" "Already running"
        return
    }

    MAIN=$(swaymsg -t get_outputs -r | jq -r \
        '.[] | select(.active and .name != "HEADLESS-1") | .name' | head -1)

    [ -z "$MAIN" ] && { notify-send "Error" "No output found"; exit 1; }

    swaymsg create_output
    sleep 0.3
    wl-mirror --fullscreen --fullscreen-output "$HEADLESS" "$MAIN" &
    echo $! > "$PID_FILE"

    notify-send "Stealth Mirror" "Share '$HEADLESS' in Zoom/Meet"
}

stop() {
    [ -f "$PID_FILE" ] && {
        kill "$(cat "$PID_FILE")" 2>/dev/null
        rm -f "$PID_FILE"
    }
    swaymsg output "$HEADLESS" unplug 2>/dev/null
    notify-send "Stealth Mirror" "Stopped"
}

case "${1:-start}" in
    start)  start ;;
    stop)   stop ;;
    toggle) [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null && stop || start ;;
esac
```

```bash
chmod +x ~/.config/sway/scripts/stealth-mirror.sh
```

### 3. Add Sway Config

Add to `~/.config/sway/config`:

```bash
# Stealth mirror hotkey
bindsym $mod+Shift+m exec ~/.config/sway/scripts/stealth-mirror.sh toggle

# Virtual monitor settings (when created)
output HEADLESS-1 resolution 1920x1080 position 0 1080 bg #000000 solid_color
```

### 4. Usage

**Before meeting:**
```bash
~/.config/sway/scripts/stealth-mirror.sh start
# Or press Mod+Shift+M
```

**In Zoom/Meet:**
1. Click "Share Screen"
2. Select **HEADLESS-1** (not your main monitor)
3. ShyQuiet is invisible to participants

**After meeting:**
```bash
~/.config/sway/scripts/stealth-mirror.sh stop
```

---

## Why It Works

```
Your Monitor (eDP-1)              Virtual Monitor (HEADLESS-1)
┌─────────────────────┐           ┌─────────────────────┐
│  [Browser]          │           │  [Browser]          │
│                     │  ──────►  │                     │
│  🤫 ShyQuiet        │  wl-mirror│  (no overlay)       │
│  (Layer::Overlay)   │           │                     │
└─────────────────────┘           └─────────────────────┘
                                           ▲
                                  Zoom shares this
```

`wl-mirror` uses `wlr-screencopy` protocol which **excludes Overlay layer by design**.

---

## Troubleshooting

**Check HEADLESS-1 exists:**
```bash
swaymsg -t get_outputs | jq '.[].name'
```

**Verify overlay is hidden:**
```bash
grim -o HEADLESS-1 /tmp/test.png && xdg-open /tmp/test.png
# ShyQuiet should NOT appear
```

**wl-mirror not found:**
```bash
which wl-mirror || echo "Install wl-mirror first"
```

