# ShyQuiet Architecture

## Overview

ShyQuiet is a privacy-first AI assistant with **stealth mode** — invisible during screen sharing on Wayland.

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
│   │   - Screenshot capture (grim/portal)                │   │
│   │   - AI/STT API calls (reqwest)                      │   │
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
│   │   ├── audio/           # Audio capture, VAD, encoding
│   │   ├── capture/         # Screenshot capture
│   │   ├── api/             # AI/STT HTTP clients
│   │   └── state/           # Shared application state
│   └── ui/
│       ├── mod.rs
│       ├── stealth.rs       # Unified stealth window (GTK4 + layer-shell)
│       └── widgets/         # Custom GTK4 widgets
├── assets/
│   ├── icons/               # App icons
│   └── shyquiet.desktop     # Desktop entry
├── docker/
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── docker-build.sh
├── scripts/
│   ├── run, debug, logs, kill
│   ├── docker, lint, fmt
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
gtk4 = "0.9"
libadwaita = "0.7"
gtk4-layer-shell = "0.4"

# Async runtime
tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }

# HTTP client (AI/STT APIs)
reqwest = { version = "0.12", features = ["json"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
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
Single window containing ALL functionality — invisible in screen share on Wayland.

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

### Wayland (Full Stealth) ✅
- Protocol: `zwlr-layer-shell-v1`
- Layer: `Overlay`
- Keyboard: `OnDemand`
- Result: NOT in compositor window list

### X11 (No Stealth) ⚠️
- X11 protocol does not support hiding windows from screen capture
- App runs as regular GTK4 window
- All features work, but window IS VISIBLE in screen share
- **Recommendation:** Use Wayland for stealth functionality

### Test Criteria (Wayland)
```bash
# Stealth window should NOT appear:
grim screenshot.png           # Screenshot
swaymsg -t get_tree          # Window tree
Zoom/Meet screen share       # Video call
OBS window capture           # Streaming
```

---

## State Management

```rust
// Shared state for all views
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<Conversation>,
    pub prompts: Vec<SystemPrompt>,
    pub current_chat: Vec<Message>,
}

// Access via Rc<RefCell<AppState>>
let state = Rc::new(RefCell::new(AppState::default()));
```

### Persistence
- **Config:** `~/.config/shyquiet/settings.json`
- **Data:** `~/.local/share/shyquiet/`
  - `history.json` - Chat history
  - `prompts.json` - System prompts

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
./dev kill      # Stop all instances
```

### Lint & Format
```bash
./dev lint      # cargo clippy
./dev fmt       # cargo fmt
```

---

