# Contributing

Bug reports and pull requests are welcome. Before a larger change, open an issue so we can agree on the approach.

## Build

Install Rust and the system libraries from the [README](README.md#install), then:

```bash
git clone https://github.com/AshBuk/Shaya
cd Shaya
cargo build --release
./dev run
```

`./dev docker` opens a menu that builds in a container, with no Rust or GTK headers on the host. Files it writes to `target/` belong to root, so pick either the container or a local build for one checkout.

## Running and debugging

`./dev debug` runs the release binary with `RUST_LOG=debug`, and `./dev logs` follows its output in `$XDG_RUNTIME_DIR/shaya/shaya.log`. Build first, both use `target/release/shaya`.

Only one instance runs at a time: starting the binary again shows or hides the window. To stop it, run `pkill -f shaya`.

To start from a clean state, remove `~/.config/shaya` and `~/.local/share/shaya`. This also deletes your API keys and history.

## Before a pull request

CI runs these checks, and they must pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Clippy treats `unwrap()` and `expect()` as errors in CI, so handle the failure instead.

Commit subjects use `scope: short imperative subject` in lowercase, for example `chat: extract markdown parsing helpers`.

Overlay behavior depends on the compositor, so say in the pull request which compositor you tested on. See the [Overlay Guide](docs/OVERLAY_GUIDE.md) for compositor setup. On Hyprland, `scripts/test-stealth` checks the configuration.

## Checking the providers

These tests send real, billed requests with short replies and no retries, so CI does not run them. A provider without a key is skipped. `SHAYA_LIVE_AUDIO` points to a short WAV with speech that contains the word "test", and a `.opus` file with the same name is tried as well.

```bash
GROQ_API_KEY=... ANTHROPIC_API_KEY=... OPENAI_API_KEY=... SHAYA_LIVE_AUDIO=speech.wav \
  cargo test --lib live_providers -- --ignored --nocapture --test-threads=1
```

`SHAYA_SERVER_URL` points to an OpenAI-compatible server such as `http://127.0.0.1:8080/v1`, and `SHAYA_SERVER_KEY` holds its key if it was started with one. The test asks the first model the server lists, and the image question passes only when that model reads images.

## Reporting a bug

Include your distribution, compositor and the relevant part of `./dev logs`. Logs can contain your own text, so read them before posting.

## More

[ARCHITECTURE](docs/ARCHITECTURE.md) describes the project structure and state management. [DEVELOPMENT](docs/DEVELOPMENT.md) lists dependencies and the system tools Shaya calls at runtime.