# ShyQuiet Scripts

Usage: `./dev <command>`

```
run      # Run ShyQuiet binary
debug    # Run with debug logging (background)
logs     # View logs in real-time
kill     # Stop running instances
docker   # Docker build helper (interactive menu)
lint     # cargo clippy
fmt      # cargo fmt
```

## Docker Build Options

Run `./dev docker` for interactive menu:

```
1) Build Docker image
2) Start container (shell)
3) Build ShyQuiet (cargo build --release)
4) Check compilation (cargo check)
5) Lint (cargo clippy)
6) Fmt (cargo fmt)
7) Stop container
8) Clean all
```

## Local Build (requires system deps)

```bash
# Fedora
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel

# Build
cargo build --release

# Run
./dev run
```
