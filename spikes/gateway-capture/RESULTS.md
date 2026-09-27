# Spike: gateway capture (FastCord #41)

Standalone throwaway crate (the repository has no workspace yet). It logs in by QR code (or takes a pasted token, issue #44), opens the main gateway, and writes scrubbed READY fixtures.

## Run

From the `FastCord` folder:

```
cargo run --release --manifest-path spikes/gateway-capture/Cargo.toml
```

Token paste (issue #44, the first-release login): set `FASTCORD_TOKEN` and remote auth is skipped. The variable is read once, unset, trimmed of whitespace and quotes, and shape-checked (three dot-separated base64url segments whose first segment decodes to a snowflake, or the legacy `mfa.` form); a bad shape exits with code 2 before any network call. PowerShell:

```
$env:FASTCORD_TOKEN = "<token copied from the Authorization header on discord.com>"
cargo run --release --manifest-path spikes/gateway-capture/Cargo.toml
Remove-Item Env:FASTCORD_TOKEN
```

First build takes about four minutes on the reference machine (rustls, rsa, regex). Outputs go to `spikes/gateway-capture/tests/fixtures/`:

- `ready.json`, `ready_supplemental.json`: scrubbed `d` payloads.
- `capture-meta.json`: capture date, capabilities, build number, HELLO interval, close code if any, list of dispatch events seen with sizes.

The user token stays in memory (`Zeroizing<String>`) and is never printed or written.

## Verified without a scan (2026-09-22)

- `wss://remote-auth-gateway.discord.gg/?v=2` with `Origin: https://discord.com` accepts the connection; `hello` has `heartbeat_interval` 41250 and `timeout_ms` between 240 s and 350 s across runs.
- `init` with base64 SPKI DER of a fresh RSA-2048 key is accepted; `nonce_proof` arrives.
- Nonce proof must be the base64url (unpadded) of the raw decrypted nonce. SHA-256 hashing it first (what the research doc said) gets close code 4002 `Handshake Error`. The research doc is corrected.
- `pending_remote_init` fingerprint equals the locally computed base64url SHA-256 of the SPKI DER.
- QR renders in the terminal with `qrcode` 0.14 `unicode::Dense1x2` (inverted colours for dark terminals), URL printed as fallback.
- Unit tests: scrubber keeps cross-references (same snowflake maps to the same placeholder everywhere, including inside CDN URLs) and replaces usernames, emails, names, content, tokens; zlib-stream inflater reassembles sync-flushed messages.

## Verified against the official web client (2026-09-22, issue #42)

The product owner's phone answered "Can't find that device" to every code the spike produced, and also to the code on `https://discord.com/login` itself, so the block is on the phone/account side, not in the spike. Before reaching that conclusion the spike was diffed against the official client:

- Official socket traffic (captured by wrapping `WebSocket` in the login page): `wss://remote-auth-gateway.discord.gg/?v=2`, `init` with `encoded_public_key`, then `{"op":"nonce_proof","nonce":<base64url of the raw decrypted nonce>}`, then `pending_remote_init`. Identical to the spike.
- Official QR: an SVG with `viewBox 0 0 37 37`; its module grid equals the `qrcode` crate's grid for `https://discord.com/ra/<fingerprint>` module for module (0 of 1369 differ).
- Handshake acceptance is loose: `nonce`+raw (official) and `proof`+SHA-256 (older write-ups) both get `pending_remote_init`; `nonce`+hashed and `proof`+raw get no reply.
- Heartbeats are acknowledged and the session stays open; the server simply never sends `pending_ticket`.
- The spike now also writes `qr.svg` next to the binary so the code can be scanned from a browser window when a terminal mangles the half-block rendering.

## Not yet verified (needs the product owner's scan, issue #42)

- `pending_ticket` user payload decryption and the `pending_login` ticket.
- `POST /users/@me/remote-auth/login` and `encrypted_token` decryption.
- Main gateway IDENTIFY with `capabilities = 1734653` being accepted (close code will be recorded in `capture-meta.json` if not).
- READY and READY_SUPPLEMENTAL shape for this capabilities value.

## Scrubber rules

- Any 17 to 20 digit string (or integer at or above 10^16) is a snowflake and maps to a stable placeholder `1000000000000000NN`.
- Values under these keys are replaced with `<key>_<n>`: username, global_name, nick, display_name, email, phone, name, topic, description, bio, content, avatar, banner, icon, splash, discovery_splash, session_id, analytics_token, auth_session_id_hash, token, vanity_url_code, pronouns, note, resume_gateway_url, rtc_regions, client_launch_id, launch_signature, id_hash, secret, unique_id, ip, address.
- Subtrees under notes, connected_accounts, user_settings_proto, user_settings, consents, experiments, guild_experiments are replaced with `"<dropped>"`.
- Any remaining token-shaped or email-shaped string is replaced with `<token>` / `<email>`.
