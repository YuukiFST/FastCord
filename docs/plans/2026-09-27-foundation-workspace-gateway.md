# Foundation: workspace + fastcord-discord gateway core Implementation Plan

> **For agentic workers:** Use `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scaffold the Cargo workspace decided in #22 and implement the pure-logic gateway core of `fastcord-discord` (Snowflake ids, token-shape login validation, gateway frames/events, resume backoff, READY two-pass join) with named fakes and synthetic fixtures, so `cargo nextest run --workspace` covers the foundation with no secrets, no network and no audio device.

**Architecture:** Six-crate workspace (`app → {discord, voice, media, storage, platform}`); `fastcord-discord` holds protocol logic with zero FastCord-specific deps. Time and randomness enter backoff/heartbeat as explicit parameters so tests never sleep. Wire frames decode to `RawFrame { op, s, t, d }` and dispatch lazily to a typed `GatewayEvent`; unknown `t` counts as unknown, never an error.

**Tech Stack:** Rust 1.98 (rust-toolchain `1.98`), tokio 1.53, tokio-tungstenite 0.30, serde/serde_json 1.0, flate2 1.1, zeroize 1.9, secrecy 0.10, rand 0.8, tracing 0.1, cargo-nextest.

**Spec:** Map #30; decisions #13 (runtime/HTTP/WS), #15 (credentials), #17 (gateway model), #22 (layout/CI/release), #23 (logging/config), #24 (test strategy); spike `spikes/gateway-capture/src/main.rs` (token shape check, IDENTIFY constants); `spikes/gateway-capture/RESULTS.md` (scrubber rules).

## Global Constraints

- Files stay under 500 lines; a module that grows past it splits (#22).
- `[workspace.lints]` denies `clippy::unwrap_used` in non-test code; test targets re-allow it explicitly (#22).
- Release profile: `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (#22).
- `CAPABILITIES: u64 = 1_734_653`, gateway URL `wss://gateway.discord.gg/?encoding=json&v=9&compress=zlib-stream`, IDENTIFY carries `token, capabilities, properties, presence, compress=false, client_state` and never `intents` (#17, spike).
- Token lives in `secrecy::SecretString` + `zeroize`; never logged (#15).
- One-command tests: `cargo nextest run --workspace`, no secrets/network/audio/manual setup (#24).
- Synthetic fixtures are marked SYNTHETIC and replaced by the #45 capture; no real token or id anywhere (#45).
- Text for the product owner in Portuguese; code, tests and commits in English (#30).

---

### Task 1: Workspace scaffold, profiles, lints, license, CI and release

**Files:**
- Create: `Cargo.toml` (workspace), `rust-toolchain.toml`, `rustfmt.toml`, `LICENSE`, `README.md`, `CHANGELOG.md`
- Modify: `.gitignore` (keep `target/`, add `*.log`, `dist/`)
- Create: `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- Create: `crates/fastcord-app/Cargo.toml`, `crates/fastcord-app/src/main.rs`
- Create: `crates/fastcord-discord/Cargo.toml`, `crates/fastcord-discord/src/lib.rs`
- Create: `crates/fastcord-voice/Cargo.toml`, `crates/fastcord-voice/src/lib.rs`
- Create: `crates/fastcord-media/Cargo.toml`, `crates/fastcord-media/src/lib.rs`
- Create: `crates/fastcord-storage/Cargo.toml`, `crates/fastcord-storage/src/lib.rs`
- Create: `crates/fastcord-platform/Cargo.toml`, `crates/fastcord-platform/src/lib.rs`

**Interfaces:**
- Consumes: nothing (first task).
- Produces: workspace members `fastcord-{app,discord,voice,media,storage,platform}`; app binary name `fastcord`; version truth `crates/fastcord-app/Cargo.toml` (`0.1.0`).

- [ ] **Step 1: Write the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/fastcord-app",
    "crates/fastcord-discord",
    "crates/fastcord-voice",
    "crates/fastcord-media",
    "crates/fastcord-storage",
    "crates/fastcord-platform",
]

[workspace.package]
edition = "2021"
rust-version = "1.95"
license = "MIT"

[workspace.dependencies]
base64 = "0.23.1"
rand = "0.8"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = { version = "1.0.151", features = ["raw_value"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread", "time", "sync", "io-std", "io-util"] }
tracing = "0.1"

[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "deny"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

[profile.dev.package."*"]
opt-level = 1
```

- [ ] **Step 2: Write `rust-toolchain.toml`, `rustfmt.toml`, extend `.gitignore`**

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.98"
components = ["rustfmt", "clippy"]
```

`rustfmt.toml` stays empty (defaults per #22). `.gitignore` becomes:

```gitignore
target/
dist/
*.log
docs/research/.issue8-comment.md
```

- [ ] **Step 3: Write `LICENSE` (MIT), `README.md`, `CHANGELOG.md`**

`LICENSE` is the standard MIT text with `Copyright (c) 2026 YuukiFST`. `README.md` states what FastCord is (native Rust Discord client for Windows, first release scope per #1), the one-command test (`cargo nextest run --workspace`), the #45 fixture note, and that no binary is signed in v1 (#37). `CHANGELOG.md` starts with `## [Unreleased]`.

- [ ] **Step 4: Write the six crate manifests and stub sources**

`crates/fastcord-app/Cargo.toml`:

```toml
[package]
name = "fastcord-app"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "fastcord"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }

[lints]
workspace = true
```

`crates/fastcord-app/src/main.rs`:

```rust
//! FastCord binary entry: builds the tokio runtime (#13) and starts the app.
//!
//! The UI thread is the winit main thread and is never a tokio worker; every
//! network/disk task runs on this runtime and reports back through channels.

use tokio::runtime::Builder;

fn main() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(3)
        .enable_all()
        .build()
        .expect("tokio runtime builds");
    runtime.block_on(async {
        tracing::debug!(version = env!("CARGO_PKG_VERSION"), "fastcord starting");
    });
}
```

`crates/fastcord-discord/Cargo.toml`:

```toml
[package]
name = "fastcord-discord"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
base64 = { workspace = true }
rand = { workspace = true }
secrecy = "0.10"
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
zeroize = "1.9.0"

[lints]
workspace = true
```

The other four crates get a manifest with only `[package]` + `[lints] workspace = true` and a `src/lib.rs` with a `//!` purpose line naming the seam (`app → {discord, voice, media, storage, platform}` per #22). `fastcord-discord/src/lib.rs`:

```rust
//! Discord protocol layer: REST, gateway, models, markdown parser,
//! permissions and rate limiter. No UI, no direct I/O.
//!
//! Crate seams follow `app → {discord, voice, media, storage, platform}` (#22).

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod auth;
pub mod gateway;
pub mod types;
```

- [ ] **Step 5: Write `.github/workflows/ci.yml` and `release.yml`**

`ci.yml` (windows-latest; cmake preinstalled; push + PR):

```yaml
name: ci
on: [push, pull_request]
jobs:
  check:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest
      - run: cargo nextest run --workspace
      - run: cargo build --release
```

`release.yml` (on tag `v*`; version truth `crates/fastcord-app/Cargo.toml`; tag must match):

```yaml
name: release
on:
  push:
    tags: ["v*"]
jobs:
  release:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Swatinem/rust-cache@v2
      - name: check tag matches crate version
        shell: pwsh
        run: |
          $v = (Select-String -Path crates/fastcord-app/Cargo.toml -Pattern '^version = "([^"]+)"').Matches.Groups[1].Value
          if ("v$v" -ne "${{ github.ref_name }}") { throw "tag ${{ github.ref_name }} != crate version v$v" }
      - run: cargo build --release
      - shell: pwsh
        run: |
          $v = (Select-String -Path crates/fastcord-app/Cargo.toml -Pattern '^version = "([^"]+)"').Matches.Groups[1].Value
          New-Item -ItemType Directory -Force dist | Out-Null
          Copy-Item target/release/fastcord.exe "dist/FastCord-$v-windows-x64.exe"
          (Get-FileHash "dist/FastCord-$v-windows-x64.exe" -Algorithm SHA256).Hash | Out-File dist/SHA256SUMS
      - run: gh release create "${{ github.ref_name }}" "dist/FastCord-*-windows-x64.exe" dist/SHA256SUMS --title "${{ github.ref_name }}" --notes-file CHANGELOG.md
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 6: Verify on the reference machine (not in this sandbox)**

Run: `cargo fmt --check` — Expected: PASS (or a diff to apply with `cargo fmt`).
Do not commit from the sandbox: no git toolchain here; commit on the reference machine.

---

### Task 2: Snowflake ids, token-shape login validation, gateway frames and backoff

**Files:**
- Create: `crates/fastcord-discord/src/types.rs`
- Create: `crates/fastcord-discord/src/auth.rs`
- Create: `crates/fastcord-discord/src/gateway.rs`
- Test: unit tests inside each module (`#[cfg(test)]`, `#![allow]` already at crate root)

**Interfaces:**
- Consumes: workspace deps from Task 1.
- Produces:
  - `types::Snowflake` — `pub struct Snowflake(pub u64)`, `FromStr` (17–20 ASCII digits), `Display`, `Serialize/Deserialize` (as JSON string).
  - `auth::validate_token_shape(SecretString) -> Result<(), TokenShapeError>` — shape check only, no network; `TokenShapeError` enum with `#[derive(Debug, PartialEq, Eq)]`.
  - `gateway::RawFrame { op: u8, s: Option<u64>, t: Option<String>, d: Box<serde_json::value::RawValue> }`, `gateway::GatewayEvent` (`Hello`, `HeartbeatAck`, `Reconnect`, `InvalidSession { resumable }`, `Dispatch { kind: EventKind, payload: Box<RawValue> }`, `Unknown { op, t: Option<String> }` — the control variants exist so the resume/heartbeat rules are testable), `gateway::EventKind` (the v1 handled names from #17), `gateway::CAPABILITIES`, `gateway::GATEWAY_URL`, `gateway::build_identify(token, properties, presence) -> Value` (never `intents`), `gateway::close_action(u16) -> CloseAction` (`Resume`, `Reidentify { wait_ms: (1000, 5000) }`, `StopAuthInvalid`, `Resume` for the rest incl. `None`/abnormal), `gateway::Backoff::next_delay_ms(now_ms: u64, rng_value: f64) -> u64` (1s,2s,4s…cap 60s, ±20% jitter, reset after 60s stable), `gateway::Heartbeat` (`new`, `first_delay_ms`, `observe_seq`, `beat() -> Option<Option<u64>>` (`None` = missed ACK, close 4000), `on_ack`, `missed`).

- [ ] **Step 1: Write the failing tests for `types::Snowflake`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn accepts_17_to_20_digit_ids() {
        assert_eq!(Snowflake::from_str("100000000000000001").unwrap().0, 1_000_000_000_000_000_01);
        assert!(Snowflake::from_str("12345678901234567890").is_ok());
    }

    #[test]
    fn rejects_short_non_numeric_and_empty() {
        for bad in ["", "123", "12a45678901234567", " 12345678901234567"] {
            assert!(Snowflake::from_str(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn serialises_as_string() {
        let s = serde_json::to_string(&Snowflake(915059169862453248)).unwrap();
        assert_eq!(s, r#""915059169862453248""#);
    }
}
```

- [ ] **Step 2: Run to verify they fail** — Run: `cargo nextest run -p fastcord-discord` — Expected: FAIL, `types` module not found.
- [ ] **Step 3: Implement `types.rs`** — struct + `FromStr` (len 17–20, all ASCII digits, parse u64) + `Display` + manual `Serialize` (string) / `Deserialize` (string or u64, rejecting < 10^16) + the tests above.
- [ ] **Step 4: Write the failing tests for `auth::validate_token_shape`** (ported from the spike's `token_shape_error`, secrets edition):

```rust
fn shape_of(hidden: &str) -> Result<(), TokenShapeError> {
    use secrecy::{ExposeSecret, SecretString};
    validate_token_shape(SecretString::from(hidden.to_owned()))
}

#[test]
fn accepts_three_segment_and_mfa_tokens() {
    // first segment is base64url("123456789012345678")
    let good = "MTIzNDU2Nzg5MDEyMzQ1Njc4.c2Vnb25kLXNlZ21lbnQtd2hpY2gtaXMtbG9uZy1lbm91Z2g.dGhpcmQtc2VnbWVudC13aGljaC1pcy1sb25nZXItc3RpbGw";
    assert_eq!(shape_of(good), Ok(()));
    assert_eq!(shape_of("mfa.abcdefghijklmnopqrstuvwxyz0123456789ABCD"), Ok(()));
}

#[test]
fn rejects_prefixes_whitespace_and_malformed() {
    assert_eq!(shape_of(""), Err(TokenShapeError::Empty));
    assert_eq!(shape_of("Bot abc.def.ghi"), Err(TokenShapeError::Prefixed));
    assert_eq!(shape_of("not a token"), Err(TokenShapeError::SegmentCount));
    assert_eq!(shape_of("aGVsbG8.c2Vnb25k.dGhpcmQtc2VnbWVudC13aGljaC1pcy1sb25n"), Err(TokenShapeError::UserId));
}
```

(`"aGVsbG8"` decodes to `"hello"`, not digits → `UserId`.)
- [ ] **Step 5: Run to verify they fail** — Expected: FAIL, `auth` module not found.
- [ ] **Step 6: Implement `auth.rs`** — port the spike logic verbatim in behaviour (trim + strip quotes, `Bot `/`Bearer ` → `Prefixed`, whitespace → `Whitespace`, `mfa.` len>20 ok, 3 segments, first segment base64url→UTF-8→17–20 digits, segments ≥6/≥20), returning `TokenShapeError::{Empty, Prefixed, Whitespace, MfaTooShort, SegmentCount, UserId, SegmentsTooShort}`; input is `secrecy::SecretString`, exposed only inside the function.
- [ ] **Step 7: Write the failing tests for `gateway.rs`** — decode a HELLO (`{"op":10,"d":{"heartbeat_interval":41250}}`), a dispatch with unknown `t` (counts as `Unknown`, never errors), close-code table (`4004 → StopAuthInvalid`, `4010 → StopAuthInvalid`, `4009 → Reidentify`, `None → Resume`), backoff sequence (`1000, 2000, 4000 …` with jitter bounds, cap `60000`, reset after 60 s stable).
- [ ] **Step 8: Run to verify they fail** — Expected: FAIL, `gateway` module not found.
- [ ] **Step 9: Implement `gateway.rs`** — `RawFrame` with `Box<RawValue>` payload, `EventKind::from_name` covering the v1 list from #17 (`READY`, `READY_SUPPLEMENTAL`, `RESUMED`, `MESSAGE_CREATE/UPDATE/DELETE/DELETE_BULK`, `MESSAGE_REACTION_ADD/REMOVE/REMOVE_ALL/REMOVE_EMOJI`, `MESSAGE_ACK`, `CHANNEL_CREATE/UPDATE/DELETE`, `CHANNEL_PINS_UPDATE`, `CHANNEL_UNREAD_UPDATE`, `THREAD_CREATE/UPDATE/DELETE/LIST_SYNC`, `GUILD_CREATE/UPDATE/DELETE`, `GUILD_ROLE_CREATE/UPDATE/DELETE`, `GUILD_MEMBER_UPDATE`, `GUILD_MEMBER_LIST_UPDATE`, `GUILD_MEMBERS_CHUNK`, `GUILD_EMOJIS_UPDATE`, `PRESENCE_UPDATE`, `TYPING_START`, `USER_UPDATE`, `USER_GUILD_SETTINGS_UPDATE`, `USER_SETTINGS_PROTO_UPDATE`, `RELATIONSHIP_ADD/REMOVE`, `VOICE_STATE_UPDATE`, `VOICE_SERVER_UPDATE`, `SESSIONS_REPLACE`), `CloseAction`, pure `Backoff` (state: `failures: u32`, `last_change_ms: u64`; `next_delay_ms(now_ms, rng01)`), `Heartbeat` (`interval_ms`, `last_seq`, `acked: bool`; `missed_ack()` true when a beat went unacked).
- [ ] **Step 10: Run** `cargo nextest run -p fastcord-discord` — Expected: PASS. Then `cargo clippy -p fastcord-discord --all-targets -- -D warnings` — Expected: PASS.

---

### Task 3: READY two-pass join, synthetic fixtures, FakeGateway replay test

**Files:**
- Create: `crates/fastcord-discord/src/ready.rs` (declared in `lib.rs`)
- Create: `crates/fastcord-discord/src/testing.rs` (`FakeGateway`, behind `#[cfg(any(test, feature = "testing"))]`, feature `testing`)
- Create: `crates/fastcord-discord/tests/fixtures/gateway/ready.json` (SYNTHETIC)
- Create: `crates/fastcord-discord/tests/fixtures/gateway/ready_supplemental.json` (SYNTHETIC)
- Create: `crates/fastcord-discord/tests/fixtures/gateway/capture-meta.json` (SYNTHETIC)
- Create: `crates/fastcord-discord/tests/fixtures/gateway/README.md`
- Create: `crates/fastcord-discord/tests/gateway_replay.rs`
- Modify: `crates/fastcord-discord/Cargo.toml` (feature `testing`, dev-dep `tokio` macros already via workspace)

**Interfaces:**
- Consumes: `types::Snowflake`, `gateway::{RawFrame, GatewayEvent}` from Task 2.
- Produces:
  - `ready::ReadySnapshot { users: HashMap<Snowflake, UserLite>, guilds: Vec<GuildLite>, private_channels: Vec<ChannelLite>, read_state: Vec<ReadEntry>, presences: Vec<Snowflake> }`
  - `ready::join_ready(ready: &RawValue, supplemental: Option<&RawValue>) -> Result<ReadySnapshot, ReadyError>` — pass one: `users` into map; pass two: `merged_members` resolved by `user_id`, `private_channels` by `recipient_ids`; supplemental merges presences (additive, membership validated); missing user reference → `ReadyError::UnknownUser(Snowflake)`.
  - `testing::FakeGateway` — `new(script: Vec<FakeStep>)`, `FakeStep::{Frame(&'static str), Close(Option<u16>)}`, implements `Iterator<Item = TransportEvent>` with `TransportEvent::{Hello(u64), Dispatched(EventKind), Closed(Option<u16>)}` (no clock or async runtime needed; control frames are consumed silently, like the gateway task consumes them).

- [ ] **Step 1: Write synthetic fixtures** — `ready.json` = scrubbed-shaped `d` payload with 2 users (`100000000000000001/02`), 1 guild (`100000000000000010`) with `merged_members` pointing at user 01, 1 DM channel with `recipient_ids: ["100000000000000002"]`, `read_state` 1 entry, `user_guild_settings` 1 entry; every id a `1000000000000000NN` placeholder per the spike scrubber rules; `capture-meta.json` with `"synthetic": true`, `"capabilities": 1734653`, `"client_build_number": 617136`, `"replaced_by_issue": 45`. `fixtures/README.md` (PT-BR, product-owner tone): these files are synthetic, the command that replaces them (`$env:FASTCORD_TOKEN = "<token>"; cargo run --release --manifest-path spikes/gateway-capture/Cargo.toml`), and never paste a token anywhere.
- [ ] **Step 2: Write the failing test** (`tests/gateway_replay.rs`, `#![allow(clippy::unwrap_used)]` at top):

```rust
#![allow(clippy::unwrap_used)]
use fastcord_discord::ready::join_ready;

#[test]
fn joins_users_members_and_dm_recipients() {
    let ready = std::fs::read_to_string("tests/fixtures/gateway/ready.json").unwrap();
    let supp = std::fs::read_to_string("tests/fixtures/gateway/ready_supplemental.json").unwrap();
    let snap = join_ready(
        &serde_json::from_str::<Box<serde_json::value::RawValue>>(&ready).unwrap(),
        Some(&serde_json::from_str(&supp).unwrap()),
    )
    .unwrap();
    assert_eq!(snap.users.len(), 2);
    assert_eq!(snap.private_channels.len(), 1);
    assert_eq!(snap.private_channels[0].recipients.len(), 1);
}

#[test]
fn unknown_member_user_is_an_error() {
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("tests/fixtures/gateway/ready.json").unwrap()).unwrap();
    v["guilds"][0]["merged_members"][0]["user_id"] = serde_json::json!("100000000000000099");
    let raw = serde_json::value::to_raw_value(&v).unwrap();
    assert!(join_ready(&raw, None).is_err());
}
```

- [ ] **Step 3: Run to verify it fails** — Run: `cargo nextest run -p fastcord-discord --test gateway_replay` — Expected: FAIL, `ready` module not found.
- [ ] **Step 4: Implement `ready.rs`** — `UserLite { id, username }`, `GuildLite { id, member_ids: Vec<Snowflake> }`, `ChannelLite { id, recipients: Vec<Snowflake> }`, `ReadEntry { channel_id: Snowflake, last_message_id: Option<Snowflake> }`; two-pass join exactly as specified; supplemental reads `merged_presences.friends` (the real `READY_SUPPLEMENTAL` nesting) and merges presences additively with membership validated.
- [ ] **Step 5: Implement `testing.rs` + `testing` feature** — `FakeGateway` replaying the script; a unit test replays `[HELLO, IDENTIFY-echo, READY, READY_SUPPLEMENTAL, Close(None)]` and asserts the decoded sequence `[Hello(41250), Dispatched(READY), Dispatched(READY_SUPPLEMENTAL), Closed]` plus `close_action(None) == Resume`.
- [ ] **Step 6: Run** `cargo nextest run --workspace` — Expected: PASS. Then `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` — Expected: PASS.

---

### Task 4: Commit, push, release

**Files:** all files from Tasks 1–3.

- [ ] **Step 1: Review the diff** — Run: `git status --short` and `git diff --stat` — Expected: only the new files above, no `target/`, no token, no real id.
- [ ] **Step 2: Commit** — Run: `git add -A && git commit -m "feat: workspace scaffold and discord gateway core" -m "Decisions #13 #15 #17 #22 #23 #24. Fixtures are synthetic until #45."` — Expected: clean commit.
- [ ] **Step 3: Push** — Run: `git push origin main` — Expected: up to date with `main`.
- [ ] **Step 4: Release (only when the product owner asks for a binary)** — Run: tag `v0.1.0` matching `crates/fastcord-app/Cargo.toml`, push the tag; `release.yml` builds `FastCord-0.1.0-windows-x64.exe` + `SHA256SUMS` on `windows-latest`.
