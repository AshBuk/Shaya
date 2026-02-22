# Shaya Architecture

## Overview

Shaya is a privacy-first AI assistant with **stealth mode** — invisible to `wlr-screencopy` based tools on Wayland (Sway, Hyprland). See [STEALTH_GUIDE.md](STEALTH_GUIDE.md) for PipeWire/Zoom limitations.

## Core Principle

**One Rust binary, one unified window:**
- **Unified Window** — GTK4 + layer-shell + libadwaita (ALL functionality in one invisible window)

**No web technologies. Pure native GTK4.**

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    SHAYA BINARY                          │
│                   (Pure Rust + GTK4)                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │            UNIFIED WINDOW                            │   │
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
│   │   - Screenshot capture (grim/spectacle/XDG Portal)  │   │
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
Shaya/
├── Cargo.toml
├── Cargo.lock
├── rustfmt.toml
├── clippy.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library root
│   ├── core/
│   │   ├── api/             # LLM + Whisper STT clients
│   │   ├── audio/           # Capture, VAD, chunking, encoding
│   │   ├── capture/         # Screenshot capture (multi-DE)
│   │   ├── state/           # AppState, Settings, persistence
│   │   ├── meeting.rs       # Meeting lifecycle, transcription, analysis
│   │   ├── models.rs        # AI/STT model constants
│   │   ├── lock.rs          # Single-instance lock + SIGUSR1 signaling
│   │   └── utils.rs         # Shared utilities
│   └── ui/
│       ├── stealth.rs       # Main window (GTK4 + layer-shell)
│       ├── layer_shell.rs   # Layer-shell init / fallback
│       ├── header.rs        # Header bar with navigation
│       └── views/
│           ├── chat/        # Chat UI, messaging, audio, markdown
│           ├── settings.rs  # API keys, model selection, theme
│           ├── history.rs   # Past conversations
│           ├── prompts.rs   # System prompts editor
│           └── meetings.rs  # Meeting recording and transcription
├── assets/
│   └── shaya.desktop     # Desktop entry
├── docker/
│   ├── Dockerfile
│   ├── docker-compose.yml
│   └── docker-build.sh
├── dev                         # Entry point: ./dev <command>
├── scripts/
│   ├── run, debug, logs        # Runtime scripts
│   ├── docker, lint, fmt       # Build/quality scripts
│   ├── test-stealth            # Stealth mode verification
│   └── README.md
└── docs/
    └── *.md
```

---

## Unified Stealth Window

### Purpose
Single window containing ALL functionality — invisible to wlr-screencopy tools (grim, slurp).

### UI Structure
```
┌──────────────────────────────────────────┐
│  🤫 Shaya  [Chat][⚙️][📜][📝][⏺]       │  ← Tab switcher
├──────────────────────────────────────────┤
│                                          │
│           Active View Content            │  ← ViewStack
│                                          │
└──────────────────────────────────────────┘
```

### Views
1. **Chat** — AI interaction, voice input, screenshots
2. **Settings** — API keys, model selection, theme, opacity
3. **History** — Past conversations (browse, continue, delete)
4. **Prompts** — System prompts editor (multiple active)
5. **Meetings** — Record, transcribe, AI analysis

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
- **Config:** `~/.config/shaya/settings.json`
- **Settings + chat state:** stored in `settings.json`
- **Meetings data:** `~/.local/share/shaya/meetings/<meeting-id>/`
  - `metadata.json`
  - `transcript.txt`
  - `analysis.md`
  - `system.wav` / `mic.wav` (or `.opus`)
