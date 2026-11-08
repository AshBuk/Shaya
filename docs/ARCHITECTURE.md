# Shaya Architecture

## Overview

Shaya is an AI assistant that runs as a compositor overlay. Whether it appears in screenshots and screen sharing depends on the compositor: see [OVERLAY_GUIDE.md](OVERLAY_GUIDE.md).

## Core Principle

**One Rust binary, one unified window:**
- **Unified Window** — GTK4 + layer-shell + libadwaita (ALL functionality in one window)

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
│   │   │  - Prompts View (system prompts)            │   │   │
│   │   │  - History View (conversations)             │   │   │
│   │   │  - Cheatsheets View (markdown notes)        │   │   │
│   │   │  - Meetings View (recordings, transcript)   │   │   │
│   │   └─────────────────────────────────────────────┘   │   │
│   │                                                     │   │
│   │   Wayland: zwlr-layer-shell-v1 → overlay surface    │   │
│   │   GNOME, X11: Fallback → normal window              │   │
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
│           ├── history.rs      # Past conversations
│           ├── prompts.rs      # System prompts editor
│           ├── cheatsheets.rs  # Quick-access markdown notes
│           └── meetings.rs     # Meeting recording and transcription
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
Single window containing ALL functionality. On Hyprland a layer rule blacks it out in captures, other compositors show it (see [OVERLAY_GUIDE.md](OVERLAY_GUIDE.md)).

### UI Structure
```
┌──────────────────────────────────────────┐
│  🤫 Shaya  [Chat][⚙️][📝][📜][📋][⏺]   │  ← Tab switcher
├──────────────────────────────────────────┤
│                                          │
│           Active View Content            │  ← ViewStack
│                                          │
└──────────────────────────────────────────┘
```

### Views
1. **Chat** — AI interaction, voice input, screenshots
2. **Settings** — API keys, model selection, theme, opacity
3. **Prompts** — System prompts editor (multiple active)
4. **History** — Past conversations (browse, continue, delete)
5. **Cheatsheets** — Quick-access markdown notes with live preview
6. **Meetings** — Record, transcribe, AI analysis

---

## State Management

```rust
// Shared state for all views
pub struct AppState {
    pub settings: Settings,
    pub history: Vec<Conversation>,
    pub prompts: Vec<SystemPrompt>,
    pub active_prompt_ids: Vec<String>,
    pub cheatsheets: Vec<Cheatsheet>,
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
  - `system.wav` / `mic.wav`, kept until transcription succeeds
  - `system.opus` / `mic.opus` / `mixed.opus`

These directories are `0700`, and `metadata.json`, `transcript.txt` and `analysis.md` are `0600`. Temporary audio and screenshots, the lock file and `./dev debug` logs live in `$XDG_RUNTIME_DIR/shaya` (or `~/.cache/shaya`).
