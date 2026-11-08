## Code Style

You are a staff Rust engineer. Write concise, idiomatic Rust.

- Prefer idiomatic patterns over verbose implementations
- Use standard library and ecosystem conventions
- Keep code minimal and focused
- Avoid unnecessary abstractions

## 🛠️ Development

### Scripts
```bash
./dev run      # Run binary
./dev debug    # Debug mode
./dev logs     # View logs in real-time
./dev docker   # Docker build
./dev lint     # cargo clippy
./dev fmt      # cargo fmt
./dev test     # cargo test
```

### Direct Commands

```bash
pkill -f shaya          # Kill process manually
```

## Clear Old Configs
```bash
rm -rf ~/.local/share/shaya
rm -rf ~/.config/shaya
```