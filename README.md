<div align="center">
<img src="icons/shaya.svg" width="180" height="180" alt="Shaya"/>

# Shaya

**Native AI assistant for Linux, built with Rust and GTK4.**

</div>

Shaya lives as an overlay on desktop and gives you instant access to AI chat, voice transcription, screenshots, meeting recording & analysis. A fast and quiet tool during deep work. 

On Wayland it runs as a compositor overlay.

One Rust binary, pure GTK4.

## Features

| | |
|---|---|
| **AI Chat** | Groq, OpenAI, Anthropic or a local model through any OpenAI-compatible server. Conversation with context, markdown rendering, system prompts |
| **Voice Capture** | Record microphone or system audio, automatic transcription |
| **Screenshots** | Full screen or area selection, auto-attached to chat as context |
| **Meeting Recorder** | Capture mic + system audio, transcribe, get AI analysis |
| **Prompt Library** | Manage and switch system prompts for different scenarios |
| **Cheatsheets** | Quick-access markdown notes with live preview |
| **Conversation History** | Browse, continue, or delete past sessions |
| **Appearance** | Adjustable opacity, light/dark theme |
| **Quick Toggle** | Running `shaya` again shows or hides the window, see [OVERLAY_GUIDE](docs/OVERLAY_GUIDE.md#toggle-shortcut) |
| **Privacy** | BYOK (bring your own key): no account and no Shaya servers. Requests go directly to the provider you choose or to a server you run, everything else stays on your machine |
| **Models** | Pick from the built-in list or enter any model ID. A text model can pass images to a model of any cloud provider |
| **Stealth Mode** | Wayland overlay. Hiding from screen capture depends on the compositor |

Bring your own API key for Anthropic, OpenAI or Groq. For chat you can also point Shaya at an OpenAI-compatible server such as llama.cpp, Ollama, LM Studio or vLLM: set its URL in Settings, the key is optional, and the model list comes from the server. The default `http://127.0.0.1:8080/v1` is the llama.cpp port. Ollama listens on 11434, LM Studio on 1234 and vLLM on 8000, so for them change the port and keep `/v1`. Screenshots reach a local model when it reads images, for llama.cpp that means a server started with `--mmproj`.

Speech-to-text goes through Groq or OpenAI, so voice capture, meetings and Live still need one of their keys. Groq is recommended for STT and fast responses.

---

## Why Shaya Was Built

Most existing AI assistants of that class ship bloated Electron/WebView wrappers that eat RAM and feel alien on Linux.

Shaya is a tool built for Linux users who care about their stack:

- **GTK4 + libadwaita** — GNU/GNOME native, first-class Wayland citizen
- **Layer Shell** — compositor-level overlay via wlr-layer-shell protocol
- **Privacy-first** — your API keys stay local, nothing phones home
- **Single binary**

#### Overlay Behavior

Shaya uses Wayland layer-shell to render as a compositor overlay. How it appears during screen sharing depends on your compositor:

> See the [Overlay Guide](docs/OVERLAY_GUIDE.md) for compositor setup and screen sharing.

---

## Install

Shaya builds from source into one binary. COPR and AUR packages are planned.

Install [Rust](https://rustup.rs) and the system libraries:

```bash
# Fedora
sudo dnf install gcc gtk4-devel libadwaita-devel gtk4-layer-shell-devel
# Arch
sudo pacman -S --needed base-devel gtk4 libadwaita gtk4-layer-shell
# Debian 13+, Ubuntu 25.10+
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libgtk4-layer-shell-dev
```

Then build and run:

```bash
cargo install --locked --git https://github.com/AshBuk/Shaya
shaya
```

To remove it, run `cargo uninstall shaya`.

Audio and screenshots use system tools, listed in [DEVELOPMENT](docs/DEVELOPMENT.md#system-tools-at-runtime).

---

## Recording Other People

Tell the participants before you record a meeting or capture system audio. Laws differ by country, and some require consent from everyone on the call.

---

## Documentation

- [CONTRIBUTING](CONTRIBUTING.md): local development, checks before a pull request
- [ARCHITECTURE](docs/ARCHITECTURE.md): project structure, design decisions, state management
- [DEVELOPMENT](docs/DEVELOPMENT.md): dependencies, build setup, scripts

### [Apache-2.0](LICENSE) LICENSE

---

Sharing with the community for privacy-conscious Linux users.

Link back to this repo and ⭐ it if it helped you.

[![Sponsor](https://img.shields.io/badge/Sponsor-💖-pink?style=for-the-badge&logo=github)](https://github.com/sponsors/AshBuk) [![PayPal](https://img.shields.io/badge/PayPal-00457C?style=for-the-badge&logo=paypal&logoColor=white)](https://www.paypal.com/donate/?hosted_button_id=R3HZH8DX7SCJG)
