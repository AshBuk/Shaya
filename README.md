<div align="center">
<img src="icons/shaya.svg" width="180" height="180" alt="Shaya"/>

# Shaya

**AI assistant overlay for Linux with cloud or local models. Native GTK4, Rust, Wayland.**

[![CI](https://github.com/AshBuk/Shaya/actions/workflows/ci.yml/badge.svg)](https://github.com/AshBuk/Shaya/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/AshBuk/Shaya?sort=semver)](https://github.com/AshBuk/Shaya/releases)
[![Rust](https://img.shields.io/badge/rust-1.89-orange?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/github/license/AshBuk/Shaya)](LICENSE)

[![AppImage](https://img.shields.io/badge/AppImage-available-0a7cff?logo=appimage)](https://github.com/AshBuk/Shaya/releases/latest)
[![AUR](https://img.shields.io/aur/version/shaya?logo=archlinux)](https://aur.archlinux.org/packages/shaya)
[![COPR](https://img.shields.io/badge/COPR-Fedora-51a2da?logo=fedora)](https://copr.fedorainfracloud.org/coprs/ashbuk/shaya/)

</div>

Shaya lives as an overlay on desktop and gives you instant access to AI chat, voice transcription, screenshots, meeting recording & analysis. Use a cloud provider with your own key or a model running on your machine. A fast and quiet tool during deep work.

On Wayland it runs as a compositor overlay.

One Rust binary, pure GTK4.

https://github.com/user-attachments/assets/f784e2ab-19d0-4e37-95b7-e8a60eb7ab86

## Features

| | |
|---|---|
| **AI Chat** | Groq, OpenAI, Anthropic or a local model through any OpenAI-compatible server. Conversation with context, markdown rendering, system prompts |
| **Voice Capture** | Record microphone or system audio, automatic transcription |
| **Screenshots** | Full screen or area selection, auto-attached to chat as context |
| **Meeting Recorder** | Capture mic + system audio, transcribe, get AI analysis |
| **Live Assistant** | Listens to a call, transcribes it in short chunks and suggests what to say when a question comes up |
| **Prompt Library** | Manage and switch system prompts for different scenarios |
| **Cheatsheets** | Quick-access markdown notes with live preview |
| **Conversation History** | Browse, continue, or delete past sessions |
| **Appearance** | Adjustable opacity, light/dark theme |
| **Quick Toggle** | Running `shaya` again shows or hides the window, see [OVERLAY_GUIDE](docs/OVERLAY_GUIDE.md#toggle-shortcut) |
| **Privacy** | BYOK (bring your own key): no account and no Shaya servers. Requests go directly to the provider you choose or to a server you run, everything else stays on your machine |
| **Models** | Pick from the built-in list or enter any model ID. A text model can pass images to a model of any cloud provider |
| **Stealth Mode** | Wayland overlay. Hiding from screen capture depends on the compositor, see [OVERLAY_GUIDE](docs/OVERLAY_GUIDE.md) |

## Providers

Requests go straight from Shaya to the provider, with your own key.

| Provider | Chat | Speech-to-text |
|---|---|---|
| Groq | yes | yes |
| OpenAI | yes | yes |
| Anthropic | yes | no |
| OpenAI-compatible server | yes | no |

Voice capture, meetings and Live Assistant need speech-to-text, so they take a Groq or OpenAI key. Groq is recommended for STT and fast responses.

### Local models

Choose OpenAI-compatible as the provider in Settings and enter the server URL. The key is optional, and the model list comes from the server.

| Server | URL |
|---|---|
| llama.cpp | `http://127.0.0.1:8080/v1`, the default |
| Ollama | `http://127.0.0.1:11434/v1` |
| LM Studio | `http://127.0.0.1:1234/v1` |
| vLLM | `http://127.0.0.1:8000/v1` |

Screenshots reach a local model when it reads images. For llama.cpp that means a server started with `--mmproj`, or a model loaded with `-hf` that ships one. For a text-only model, turn off Reads Images in Settings and screenshots go to the image model of a cloud provider.

## ✦ Installation

### Fedora [COPR](https://copr.fedorainfracloud.org/coprs/ashbuk/shaya/)

Fedora 44 and newer:

```bash
sudo dnf copr enable ashbuk/shaya
sudo dnf install shaya
```

### Arch Linux [AUR](https://aur.archlinux.org/packages/shaya)

```bash
yay -S shaya
```

### AppImage

Download `Shaya-*-x86_64.AppImage` from [Releases](https://github.com/AshBuk/Shaya/releases/latest) and put it on your `PATH` as `shaya`, so the shortcut command stays the same:

```bash
install -m755 Shaya-*-x86_64.AppImage ~/.local/bin/shaya
```

The AppImage carries update information for AppImageUpdate.

Audio and screenshots use system tools, listed in [DEVELOPMENT](docs/DEVELOPMENT.md#system-tools-at-runtime). To build from source, see [DEVELOPMENT](docs/DEVELOPMENT.md#build).

## Recording Other People

Tell the participants before you record a meeting or capture system audio. Laws differ by country, and some require consent from everyone on the call.

## Documentation

- [CONTRIBUTING](CONTRIBUTING.md): local development, checks before a pull request
- [ARCHITECTURE](docs/ARCHITECTURE.md): project structure, design decisions, state management
- [DEVELOPMENT](docs/DEVELOPMENT.md): dependencies, build setup, scripts

## ✦ Apache-2.0 [LICENSE](LICENSE)

---

Sharing with the community for privacy-conscious Linux users.

Link back to this repo and ⭐ it if it helped you.

[![Sponsor](https://img.shields.io/badge/Sponsor-💖-pink?style=for-the-badge&logo=github)](https://github.com/sponsors/AshBuk) [![PayPal](https://img.shields.io/badge/PayPal-00457C?style=for-the-badge&logo=paypal&logoColor=white)](https://www.paypal.com/donate/?hosted_button_id=R3HZH8DX7SCJG)
