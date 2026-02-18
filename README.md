# Shaya

**Native AI assistant for Linux.**

Your quiet tool during deep work, meetings, interviews etc. Shaya lives as an overlay on your desktop — invisible to screen sharing on Wayland — and gives you instant access to AI chat, voice transcription, and screenshots — only you know it's there.

One Rust binary. Pure GTK4. No Electron. No web views. No compromises.

---

## Why Shaya Was Built

Existing AI assistants (Cluely, etc.) ship bloated Electron wrappers that eat RAM, leak into screen shares, and feel alien on Linux.

Shaya is a **boutique tool** built for Linux users who care about their stack:

- **GTK4 + libadwaita** — first-class GNOME/Wayland citizen
- **Layer Shell** — true compositor-level invisibility, not a hack
- **Single binary** — no runtime, no daemon, no dependencies beyond GTK
- **Privacy-first** — your API keys stay local, nothing phones home

---

## Features

| | |
|---|---|
| **AI Chat** | Conversation with context, markdown rendering, system prompts |
| **Voice Capture** | Record microphone or system audio, automatic transcription |
| **Screenshots** | Full screen or area selection, auto-attached to chat as context |
| **Meeting Recorder** | Capture mic + system audio, transcribe, get AI analysis |
| **Prompt Library** | Manage and switch system prompts for different scenarios |
| **Conversation History** | Browse, continue, or delete past sessions |
| **Stealth Mode** | Invisible overlay on Wayland — hidden from Zoom, Meet, OBS |
| **Quick Toggle** | `Super+A` to show/hide instantly; adjustable opacity, light/dark theme |

---

## Overlay Behavior

Shaya uses Wayland layer-shell to render as a compositor overlay. How it appears during screen sharing depends on your compositor:

| Compositor | Overlay | Notes |
|------------|---------|-------|
| **Hyprland** | Fully hidden | Requires a one-line compositor rule |
| **Sway** | Stays out of Alt+Tab | Visible in screen capture |
| **KDE Plasma** | Stays out of Alt+Tab | Visible in screen capture |
| **GNOME Wayland** | Regular window | No layer-shell; share a specific app window instead |

> See [Stealth Guide](docs/STEALTH_GUIDE.md) for compositor setup and workarounds.

---

## Quick Start

### Build (Docker)

```bash
./dev docker
```

### Build (local, Fedora)

```bash
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel
cargo build --release
```

### Run

```bash
./dev run
```

---

## Stack

| Layer | Technology |
|-------|------------|
| Language | Rust |
| UI | GTK4 + libadwaita |
| Stealth | gtk4-layer-shell (`zwlr-layer-shell-v1`) |
| AI | Groq API (LLM + Whisper STT) |
| Audio | PipeWire / PulseAudio via `parecord` |
| Screenshots | grim, spectacle, gnome-screenshot, XDG Portal |
| HTTP | ureq (blocking, background threads) |

---

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — project structure, design decisions, state management

---

## License

[Apache-2.0](LICENSE)

---

