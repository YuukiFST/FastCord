# FastCord

Native, lightweight and fast Discord client for Windows, written in Rust.
First release scope: text, DMs, media and voice with push-to-talk (issue #1).

## Status

Foundation increment: workspace scaffold plus the `fastcord-discord` gateway
core. The UI, voice, media, storage and platform crates are skeletons; the
gateway fixtures under `crates/fastcord-discord/tests/fixtures/gateway/` are
**synthetic** and will be replaced by the real capture in issue #45.

## Requirements

- Windows 10+ (first release is Windows-only, issue #2).
- Rust stable via `rustup` (`rust-toolchain.toml` pins 1.98).
- CMake on PATH (Opus codec; preinstalled on the `windows-latest` CI image).
- Visual Studio Build Tools 2022 with the C++ workload.

## Run everything

One command, no secrets, no network, no audio device, no manual setup:

```powershell
cargo nextest run --workspace
```

`cargo test --workspace` is the fallback and must also pass. CI runs exactly
this on `windows-latest`, plus `cargo fmt --check`, `cargo clippy
--workspace --all-targets -D warnings` and `cargo build --release`.

## Portable release

No installer. Tagged `v*` builds attach `FastCord-<version>-windows-x64.exe`
plus `SHA256SUMS` to the GitHub Release. The binary is not code-signed in v1,
so SmartScreen warns on first run (issue #37).

## Repo layout

```text
crates/fastcord-app/       bin: runtime entry, UI, state slices
crates/fastcord-discord/   REST, gateway, models (no UI, no direct I/O)
crates/fastcord-voice/     voice gateway, audio threads
crates/fastcord-media/     download, decode, texture cache
crates/fastcord-storage/   config.toml, state.json, disk cache, migrations
crates/fastcord-platform/  Windows only: keyring, toast, tray, hotkeys
docs/research/             Discord API research findings
docs/plans/                implementation plans
spikes/                    throwaway viability experiments
```
