# Development

### Cargo (Rust) Dependencies

| Crate | Purpose |
|-------|---------|
| `gtk4` | UI framework |
| `libadwaita` | GNOME HIG widgets, dark mode |
| `gtk4-layer-shell` | Wayland layer-shell (stealth) |
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
- `grim` + `slurp` — Hyprland, Sway
- `spectacle` — KDE Plasma
- XDG Desktop Portal — GNOME Wayland (no extra tools needed)

### System packages

**Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel
```

**Ubuntu/Debian:**
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
