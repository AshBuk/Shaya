# ShyQuiet Architecture

## Overview

ShyQuiet is a privacy-first AI assistant with **stealth mode** — invisible to `wlr-screencopy` based tools on Wayland (Sway, Hyprland). See [STEALTH_GUIDE.md](STEALTH_GUIDE.md) for PipeWire/Zoom limitations.

## Core Principle

**One Rust binary, one unified stealth window:**
- **Unified Stealth Window** — GTK4 + layer-shell + libadwaita (ALL functionality in one invisible window)

**No web technologies. Pure native GTK4.**

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    SHYQUIET BINARY                          │
│                   (Pure Rust + GTK4)                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │         UNIFIED STEALTH WINDOW                      │   │
│   │       GTK4 + gtk4-layer-shell + libadwaita          │   │
│   │                                                     │   │
│   │   ┌─────────────────────────────────────────────┐   │   │
│   │   │  ViewStack with tabs:                       │   │   │
│   │   │  - Chat View (AI interaction)               │   │   │
│   │   │  - Settings View (API keys, models)         │   │   │
│   │   │  - History View (conversations)             │   │   │
│   │   │  - Prompts View (system prompts)            │   │   │
│   │   │  - Meetings View (recordings, transcript)   │   │   │
│   │   └─────────────────────────────────────────────┘   │   │
│   │                                                     │   │
│   │   Wayland: zwlr-layer-shell-v1 → INVISIBLE          │   │
│   │   X11: Fallback → VISIBLE (no stealth possible)     │   │
│   └─────────────────────────────────────────────────────┘   │
│                            ↕                                │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              RUST CORE                              │   │
│   │                                                     │   │
│   │   - Audio capture (PipeWire/PulseAudio)             │   │
│   │   - Screenshot capture (grim/gnome-screenshot/etc)  │   │
│   │   - AI/STT API calls (ureq, blocking)               │   │
│   │   - State management (Rc<RefCell<State>>)           │   │
│   │   - Settings persistence (XDG dirs)                 │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
ShyQuiet/
├── Cargo.toml
├── Cargo.lock
├── rustfmt.toml
├── clippy.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library root
│   ├── core/
│   │   ├── audio/           # Audio capture, VAD, chunking, encoding
│   │   ├── capture/         # Screenshot capture
│   │   ├── api/             # AI/STT HTTP clients
│   │   ├── state/           # Shared application state
│   │   ├── meeting.rs       # Meeting persistence/transcription/analysis
│   │   └── lock.rs          # Single-instance lock + SIGUSR1 signaling
│   └── ui/
│       ├── mod.rs
│       ├── stealth.rs       # Unified stealth window (GTK4 + layer-shell)
│       ├── layer_shell.rs   # Layer-shell initialization/fallback
│       ├── header.rs        # Header/navigation
│       └── views/           # Chat, Settings, History, Prompts, Meetings
├── assets/
│   └── shyquiet.desktop     # Desktop entry
├── docker/
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── docker-build.sh
├── dev                         # Entry point: ./dev <command>
├── scripts/
│   ├── run, debug, logs        # Runtime scripts
│   ├── docker, lint, fmt       # Build/quality scripts
│   ├── build-layer-shell       # GTK4 layer-shell from source
│   ├── test-stealth            # Stealth mode verification
│   └── README.md
└── docs/
    └── *.md
```

---

## Dependencies

### Cargo.toml
```toml
[dependencies]
# GTK4 + Adwaita (UI)
gtk4 = "0.10"
libadwaita = { version = "0.8", features = ["v1_5"] }
gtk4-layer-shell = "0.7"

# HTTP client (AI/STT APIs) - blocking, used from background threads
ureq = { version = "2.10", features = ["json"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }
nix = { version = "0.29", features = ["signal"] }
```

### System (Fedora/Ubuntu)
```bash
# Fedora
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel

# Ubuntu (needs PPA or build from source)
sudo apt install libgtk-4-dev libadwaita-1-dev
# gtk4-layer-shell: build from source
```

---

## Unified Stealth Window

### Purpose
Single window containing ALL functionality — invisible to wlr-screencopy tools (grim, slurp).

### UI Structure
```
┌────────────────────────────────────┐
│  🤫 ShyQuiet    [Chat][⚙️][📜][📝] │  ← Tab switcher
├────────────────────────────────────┤
│                                    │
│         Active View Content        │  ← ViewStack
│                                    │
└────────────────────────────────────┘
```

### Views
1. **Chat View** — AI interaction, transcription display
2. **Settings View** — API keys, model selection, theme
3. **History View** — Past conversations
4. **Prompts View** — System prompts editor
5. **Meetings View** — Recordings, transcript, AI analysis

### Features
- AI chat interface
- Voice transcription display
- Audio capture control
- Screenshot with auto-attach
- Settings management
- Chat history
- System prompts editor

---

## Stealth Mode

### How It Works
- Protocol: `zwlr-layer-shell-v1`
- Layer: `Overlay`
- Keyboard: `OnDemand` (focus on click)

### Stealth Compatibility

| Capture Method | Stealth | Notes |
|----------------|---------|-------|
| `grim` (wlr-screencopy) | ✅ Hidden | Protocol excludes Overlay |
| `swaymsg -t get_tree` | ✅ Hidden | Layer surfaces not in tree |
| Zoom/Meet/Teams (PipeWire) | ❌ Visible | PipeWire gets full framebuffer |
| OBS PipeWire capture | ❌ Visible | Same limitation |

### Solutions for Full Screen Share

1. **Share specific window** — Select browser/IDE window instead of entire screen
2. **Virtual monitor** — Use `wl-mirror` to create clean output without overlay

See [STEALTH_GUIDE.md](STEALTH_GUIDE.md) for virtual monitor setup.

### Platform Support

| Platform | Stealth |
|----------|---------|
| Wayland + wlr-layer-shell (Sway, Hyprland) | ✅ Partial (see table above) |
| GNOME Wayland | ❌ No layer-shell support |
| X11 | ❌ No stealth possible |

---

## State Management

```rust
// Shared state for all views
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<Conversation>,
    pub prompts: Vec<SystemPrompt>,
    pub active_prompt_ids: Vec<String>,
    pub current_messages: Vec<Message>,
}

// Access via Rc<RefCell<AppState>>
let state = Rc::new(RefCell::new(AppState::default()));
```

### Persistence
- **Config:** `~/.config/shyquiet/settings.json`
- **Settings + chat state:** stored in `settings.json`
- **Meetings data:** `~/.local/share/shyquiet/meetings/<meeting-id>/`
  - `metadata.json`
  - `transcript.txt`
  - `analysis.md`
  - `system.wav` / `mic.wav` (or `.opus`)

---

## Development

### Build
```bash
# Docker (recommended)
./dev docker    # Select: Build ShyQuiet

# Local (requires system deps)
cargo build --release
```

### Run
```bash
./dev run       # Run binary
./dev debug     # Run with debug logging
./dev logs      # View logs
```

### Lint & Format
```bash
./dev lint      # cargo clippy
./dev fmt       # cargo fmt
```

---

