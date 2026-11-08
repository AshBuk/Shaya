# Development

### Cargo (Rust) Dependencies

| Crate | Purpose |
|-------|---------|
| `gtk4` | UI framework |
| `libadwaita` | GNOME HIG widgets, dark mode |
| `gtk4-layer-shell` | Wayland layer-shell overlay |
| `serde` / `serde_json` | Settings serialization |
| `anyhow` | Error handling |
| `tracing` / `tracing-subscriber` | Structured logging |
| `dirs` | XDG directory paths |
| `ureq` | Blocking HTTP (AI/STT APIs) |
| `hound` | WAV audio processing |
| `base64` | Image encoding for API |
| `libc` / `nix` | Unix signals, single-instance |
| `chrono` | Date/time formatting |

### System tools at runtime

**Audio capture** (at least one):
- `pw-record` (PipeWire) — preferred
- `parecord` (PulseAudio) — fallback

**Audio encoding** (for meeting recordings, at least one):
- `opusenc` (opus-tools) — preferred
- `ffmpeg` — fallback

**Screenshots** (depends on compositor):

| DE | Display | Area Capture | Full Screenshot |
|----|---------|--------------|-----------------|
| Hyprland | Wayland | slurp + grim (sync) | grim (sync) |
| Sway | Wayland | slurp + grim (sync) | grim (sync) |
| GNOME | Wayland | XDG Portal (async) | XDG Portal (async) |
| KDE Plasma | Wayland | spectacle -r (sync) | spectacle (sync) |
| GNOME | X11 | gnome-screenshot -a | gnome-screenshot |
| KDE | X11 | spectacle -r | spectacle |
| Other | X11 | gnome-screenshot / spectacle | gnome-screenshot / scrot / spectacle |

> **Note:** On GNOME Wayland both buttons (area and full screenshot) open the native GNOME
> screenshot dialog via the XDG Desktop Portal (`interactive` mode). The user picks
> full screen, window, or area within the portal UI.

### System packages

**Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel
```

**Debian 13+, Ubuntu 25.10+:**
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libgtk4-layer-shell-dev
```

---

## Build

### Docker cli interface
```bash
./dev docker
```

### Local
```bash
cargo build --release
```

---

## Run

```bash
./dev run          # Run (release mode)
./dev debug        # Debug mode (RUST_LOG=debug)
./dev logs         # Tail log output
```

---

## Code Quality

```bash
./dev lint         # cargo clippy
./dev fmt          # cargo fmt
```

---

## Scripts

All scripts are in `scripts/` and invoked via the `./dev` entry point. See [scripts/README.md](../scripts/README.md) for details.
