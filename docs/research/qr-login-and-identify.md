# QR login (Remote Auth) and Gateway IDENTIFY for personal Discord accounts

Research date: 2026-09-21.
Scope: what FastCord (native Rust/egui client for personal accounts, Windows-first) must implement to log a user in via the QR code flow and then open a main-gateway session as that user.

Provenance labels used throughout:

- **OFFICIAL** — documented by Discord at discord.com/developers/docs.
- **REVERSE-ENGINEERED** — from discord-userdoccers (docs.discord.food, mirror docs.discord.sex) or from the source of maintained user-account clients (discord.py-self).
  Discord does not support, document, or guarantee any of it, and it can change without notice.

Everything specific to *user* accounts in this document is REVERSE-ENGINEERED.
Discord's official docs describe the bot API only; the user API is undocumented by design.

## 1. Remote auth WebSocket handshake

**REVERSE-ENGINEERED** (userdoccers, Remote Authentication / Desktop).

The client that wants to be logged in (userdoccers calls it the "desktop" side — that is FastCord) opens a WebSocket to a static URL and must pass the API version as a query parameter:

```
wss://remote-auth-gateway.discord.gg/?v=2
```

The connection requires an `Origin` header, and the value must be one of `https://discord.com`, `https://ptb.discord.com`, or `https://canary.discord.com`.
A plain WebSocket client that omits `Origin` will be rejected, so the Rust WebSocket layer has to allow setting custom handshake headers (tokio-tungstenite's `IntoClientRequest` path does).

This gateway does not use the numeric opcodes of the main gateway.
Payloads are JSON objects whose `op` is a lower-case, under_score string, and there is no `d` envelope — fields sit at the top level.

Hello (received first, immediately after connect):

```json
{
  "op": "hello",
  "timeout_ms": 142637,
  "heartbeat_interval": 41250
}
```

`heartbeat_interval` is in milliseconds; `timeout_ms` is how long the whole remote-auth session lives before the server drops it (the QR code expires with it).
The first heartbeat should be offset by a random value between 0 and `heartbeat_interval` ms, then sent on a fixed interval.

Heartbeat (sent) and its acknowledgement (received):

```json
{ "op": "heartbeat" }
```

```json
{ "op": "heartbeat_ack" }
```

Init (sent by FastCord right after `hello`, carrying the public key):

```json
{
  "op": "init",
  "encoded_public_key": "[base64-encoded SPKI DER]"
}
```

Nonce proof — the server challenges the client to prove it owns the private key:

```json
{
  "op": "nonce_proof",
  "encrypted_nonce": "[base64-encoded RSA-OAEP ciphertext]"
}
```

```json
{
  "op": "nonce_proof",
  "nonce": "[base64URL-encoded SHA-256 of the decrypted nonce]"
}
```

Pending remote init — the server accepts the proof and returns the fingerprint that goes into the QR code:

```json
{
  "op": "pending_remote_init",
  "fingerprint": "[base64URL-encoded SHA-256 digest of the public key]"
}
```

Pending ticket — a logged-in mobile client scanned the code; the server sends back encrypted identity data so FastCord can display "is this you?":

```json
{
  "op": "pending_ticket",
  "encrypted_user_payload": "[base64-encoded RSA-OAEP ciphertext]"
}
```

Pending login — the user tapped approve on the phone:

```json
{
  "op": "pending_login",
  "ticket": "[authentication ticket]"
}
```

Cancel — the user denied the request on the phone, or the session was cancelled:

```json
{ "op": "cancel" }
```

After `pending_login` the WebSocket's job is done; the ticket is redeemed over REST (section 4).

## 2. Key exchange

**REVERSE-ENGINEERED** (userdoccers, Remote Authentication / Desktop).

FastCord generates an RSA keypair locally, per remote-auth session, and never persists it:

- RSA-OAEP, 2048-bit modulus, public exponent 65537.
- Padding OAEP with MGF1, hash SHA-256 for both the OAEP hash and the MGF1 hash, empty label.
- The public key is exported as SPKI in DER form and base64-encoded (standard alphabet, with padding) into `encoded_public_key`.

`nonce_proof` works like this: `encrypted_nonce` is base64-decoded, decrypted with the private key using the same OAEP parameters, and the *plaintext nonce bytes* are base64url-encoded **without padding** and sent back as `nonce`.
**Correction 2026-09-22 (gateway spike, issue #41):** an earlier revision of this document said to SHA-256 the nonce first; the live server answers that with close code 4002 `Handshake Error`. Only the fingerprint is a SHA-256 digest.
Note the asymmetry that is easy to get wrong: ciphertexts on the wire are standard base64, while the proof and the fingerprint are base64url without padding.

`encrypted_user_payload` decrypts to a UTF-8 string with four colon-separated fields:

```
id:discriminator:avatar:username
```

Example from the docs: `852892297661906993:0:05145cc5646fbcba277b6d5ea2030610:dolfies`.
`discriminator` is `0` for migrated (pomelo) usernames, `avatar` is the avatar hash or `0` when the user has no custom avatar.
Since usernames may themselves not contain `:`, splitting on the first three colons is safe, but treating the payload as exactly four fields is what the documented format implies.

## 3. QR content

**REVERSE-ENGINEERED** (userdoccers, Remote Authentication / Mobile).

The QR code encodes a plain URL:

```
https://discord.com/ra/<fingerprint>
```

`<fingerprint>` is exactly the string received in `pending_remote_init`, which is the base64url (unpadded) SHA-256 digest of the raw DER SPKI public key bytes.
FastCord can therefore render the QR as soon as `pending_remote_init` arrives; it does not have to compute the fingerprint itself, though computing it independently is a cheap sanity check that the server saw the right key.

Terminology warning: this "fingerprint" is unrelated to the `X-Fingerprint` tracking snowflake obtained from the experiments endpoint (section 7).
Two different things with the same name.

## 4. Token acquisition

**REVERSE-ENGINEERED** (userdoccers, Remote Authentication).

The ticket from `pending_login` is redeemed with an unauthenticated REST call:

```
POST https://discord.com/api/v9/users/@me/remote-auth/login
Content-Type: application/json

{ "ticket": "<ticket from pending_login>" }
```

The response body is:

```json
{ "encrypted_token": "[base64-encoded RSA-OAEP ciphertext]" }
```

Base64-decode `encrypted_token` and decrypt it with the same private key and the same OAEP/SHA-256 parameters; the plaintext is the user's authentication token as an ASCII string.

For completeness, the endpoints on the *other* (already authenticated, phone) side of the flow are `POST /users/@me/remote-auth` with body `{"fingerprint": "..."}` returning `handshake_token`, then `POST /users/@me/remote-auth/finish` with `{"handshake_token": "...", "temporary_token": false}` returning 204, and `POST /users/@me/remote-auth/cancel` with `{"handshake_token": "..."}` returning 204.
FastCord does not call these unless it later wants to act as the scanning device.

## 5. Token handling

**REVERSE-ENGINEERED** (userdoccers, Authentication).

A user token is not a JWT and is not a bot token from the developer portal.
Observed shape is three dot-separated base64url segments, e.g. `ODUyODkyMjk3NjYxOTA2OTkz.GX5Xdp.22jsdSqEiHLUYEJSsjeq_vJKLpOofd5QMksqw32e`, where the first segment is the base64 of the user's snowflake ID.
The remaining segments are opaque; FastCord must treat the whole string as opaque and must not attempt to parse or validate it beyond "non-empty".

For user accounts the token is sent raw in the `Authorization` header — **no `Bot ` or `Bearer ` prefix**.
Same value goes in the gateway IDENTIFY `token` field.

Lifetime: there is no documented expiry timestamp, and in practice tokens remain valid for a long time.
They are invalidated by explicit logout (`POST /auth/logout` invalidates the session), by a password change or reset (which kills all tokens and all active gateway sessions), and by account-recovery flows after a takeover.
There is no refresh endpoint in the OAuth sense.
The one refresh-like mechanism is the gateway capability `AUTH_TOKEN_REFRESH` (bit 8), which lets the server hand the client a replacement token inside the READY event; a client that enables that bit must persist the new token when READY carries one.
**Insufficient data** on the exact READY field name for the refreshed token — verify against a live READY before relying on it.

Storage on Windows: encrypt the token at rest with DPAPI (`CryptProtectData` / `CryptUnprotectData` with `CRYPTPROTECT_UI_FORBIDDEN`, user scope, plus an optional entropy blob compiled into FastCord) and write the ciphertext under `%APPDATA%\FastCord\`, or store it in Windows Credential Manager as a generic credential (`CredWriteW`/`CredReadW`), which is DPAPI-backed anyway and gives the user a visible place to revoke it.
Either is acceptable; neither protects against malware running as the same user, so the token should also be zeroized in memory when the session ends.
Never log the token, never include it in crash reports.

## 6. Main gateway IDENTIFY for a user account

**OFFICIAL** for the transport: connect to the URL returned by Get Gateway, in practice `wss://gateway.discord.gg/?v=<version>&encoding=json` with optional `&compress=zlib-stream` (or `zstd-stream`), IDENTIFY is opcode 2, HELLO is 10 with `heartbeat_interval`, heartbeat is opcode 1, and the first heartbeat should be jittered by a random fraction of the interval.
Payloads must not exceed 15 KiB.
`etf` encoding exists but `json` is the sane choice for a Rust client.

**REVERSE-ENGINEERED** for everything user-specific below (userdoccers Gateway / Gateway Events, plus discord.py-self `discord/gateway.py` and `discord/tracking.py`).

User accounts identify with API v9 and send `capabilities`, `properties` (super properties), `presence`, `compress`, and `client_state`.
They do **not** send `intents` — intents are a bot concept; a user session receives events according to its capabilities and subscriptions instead.
Sending `intents` from a user account is itself an automation signal.

IDENTIFY fields:

| Field | Type | Notes |
|---|---|---|
| `token` | string | raw user token, no prefix |
| `capabilities` | integer | bitfield, see below |
| `properties` | object | super properties, see below |
| `presence` | object | `{status, activities, afk, since}`; often overridden by READY's `sessions` |
| `compress` | boolean | payload (per-message) compression; set `false` when using transport compression |
| `client_state` | object | cache versions, see below |
| `large_threshold` | integer | optional, 25–250, default 250 for users |

The capabilities bitfield (userdoccers):

| Bit | Value | Name |
|---|---|---|
| 0 | 1 | LAZY_USER_NOTES |
| 1 | 2 | NO_AFFINE_USER_IDS |
| 2 | 4 | VERSIONED_READ_STATES |
| 3 | 8 | VERSIONED_USER_GUILD_SETTINGS |
| 4 | 16 | DEDUPE_USER_OBJECTS |
| 5 | 32 | PRIORITIZED_READY_PAYLOAD (requires bit 4) |
| 6 | 64 | MULTIPLE_GUILD_EXPERIMENT_POPULATIONS |
| 7 | 128 | NON_CHANNEL_READ_STATES |
| 8 | 256 | AUTH_TOKEN_REFRESH |
| 9 | 512 | USER_SETTINGS_PROTO |
| 10 | 1024 | CLIENT_STATE_V2 |
| 11 | 2048 | PASSIVE_GUILD_UPDATE |
| 12 | 4096 | AUTO_CALL_CONNECT |
| 13 | 8192 | DEBOUNCE_MESSAGE_REACTIONS |
| 14 | 16384 | PASSIVE_GUILD_UPDATE_V2 |
| 15 | 32768 | CHANNEL_OBFUSCATION |
| 16 | 65536 | AUTO_LOBBY_CONNECT |

Each bit changes the *shape* of READY and of subsequent events, so the number is not cosmetic: enabling `DEDUPE_USER_OBJECTS` means user objects move into a top-level `users` array, enabling `PRIORITIZED_READY_PAYLOAD` splits READY into READY plus READY_SUPPLEMENTAL, and so on.
discord.py-self's current default (`Capabilities.default()` in `discord/flags.py`) sets bits 0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12 and 14, which is **22525**, and its comment states plainly that it matches the official client's value "for anti-spam purposes".
The userdoccers example payload shows `1734653`, a different (higher, older or newer-client) value.
The two disagree, so the value must be re-derived from a real client capture at implementation time rather than copied from this document; whatever value is chosen, FastCord's READY parser must match it bit for bit.

Super properties (`properties`) as built by discord.py-self's web fallback, which is the shape FastCord should mirror:

```json
{
  "os": "Windows",
  "browser": "Chrome",
  "device": "",
  "system_locale": "en-US",
  "browser_user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ... Chrome/<v>.0.0.0 Safari/537.36",
  "browser_version": "<v>.0.0.0",
  "os_version": "10",
  "referrer": "",
  "referring_domain": "",
  "referrer_current": "",
  "referring_domain_current": "",
  "release_channel": "stable",
  "client_build_number": 396858,
  "client_event_source": null,
  "has_client_mods": false,
  "client_launch_id": "<uuid4>",
  "client_app_state": "unfocused",
  "client_heartbeat_session_id": "<uuid4>",
  "launch_signature": "<generated>"
}
```

On the gateway only, discord.py-self merges extra keys that are not part of the REST header: `{"is_fast_connect": false, "gateway_connect_reasons": "AppSkeleton"}`.
The desktop-client variant uses `browser: "Discord Client"` and a much larger property set (native build info, `os_arch`, `distro`, and friends) that the library refuses to fabricate offline, fetching it from an external properties API instead — a signal that hand-writing desktop super properties is error-prone and that impersonating the web client is the lower-risk option.

`client_state` fields (userdoccers): `guild_versions` (map of guild id to last known version), `highest_last_message_id`, `read_state_version`, `user_guild_settings_version`, `user_settings_version`, `private_channels_version`, `api_code_version`, and `initial_guild_id`.
A fresh client sends empty/zero values and fills them in on later re-identifies to avoid re-downloading state.

A minimal, verified-shape IDENTIFY (userdoccers example, with the properties above substituted in practice):

```json
{
  "op": 2,
  "d": {
    "token": "my_token",
    "capabilities": 1734653,
    "properties": { "os": "Windows", "browser": "Chrome", "device": "" },
    "presence": { "status": "unknown", "since": 0, "activities": [], "afk": false },
    "compress": false,
    "client_state": { "guild_versions": {}, "api_code_version": 0 }
  }
}
```

For REST, the *same* super-properties JSON is base64-encoded (standard base64 of the compact JSON, no newlines) and sent as the `X-Super-Properties` header on every API request, alongside `User-Agent` matching `browser_user_agent` and `X-Discord-Locale` matching `system_locale`.
Discord parses missing properties out of the `User-Agent`, so an incoherent pair (Windows properties with a Linux UA) is worse than sending nothing.

## 7. Detection considerations

State it plainly first: **automating or reimplementing a user account client violates the Discord Terms of Service, and accounts caught doing it can be terminated.**
FastCord is a self-bot-class client in Discord's eyes regardless of how faithfully it imitates the official app.
There is no configuration that makes this compliant, and the research below is about failure modes, not about immunity.

Signals that separate a third-party client from the official one (REVERSE-ENGINEERED, from userdoccers notes and discord.py-self's countermeasures):

A stale `client_build_number` is the loudest one.
Experiments and several endpoints are gated on the build number, and one that lags the live client by weeks is trivially flagged; discord.py-self scrapes it from `https://discord.com/login` assets on every start, with a hardcoded fallback only as a last resort.
FastCord must fetch it at runtime, not bake it in.

Incoherent super properties come second: a `browser_user_agent` whose Chrome version does not match `browser_version`, `os: "Windows"` with a Mac UA, `release_channel: "stable"` with a canary build number, or a `browser: "Discord Client"` set without the full desktop property block.
The REST `X-Super-Properties` must be byte-identical to the gateway `properties` (minus the gateway-only extras), otherwise the two views of the client disagree.

Missing pre-auth tracking state: the official web client performs an unauthenticated `GET /api/v9/experiments` to obtain a fingerprint snowflake, then sends it as `X-Fingerprint` on subsequent unauthenticated requests, and carries Discord's cookies through the login flow.
A client that jumps straight to an authenticated call with no prior fingerprint or cookies has a visibly truncated request history.
Fingerprint generation is rate-limited (documented as 3 valid fingerprints per 2 minutes per IP), so this must be done once per session, not per request.

TLS/HTTP fingerprinting: discord.py-self does not use a plain HTTP stack — it uses curl-cffi and explicitly picks a Chrome/Firefox/Safari `impersonate` profile so the JA3/JA4 and HTTP/2 settings match the browser it claims to be, and it sends the matching `Sec-CH-UA*` client hints.
A Rust client using rustls with default settings will present a JA3 that matches no browser at all.
Whether Discord acts on this is unverified, but the countermeasure existing in a maintained library is evidence it matters.

Behavioural signals: heartbeats sent on an exact interval with zero jitter (both gateways expect an initial random offset), request ordering that skips the calls the real client always makes around login and READY, presence that never idles, and an `hCaptcha` challenge on password login that a non-browser client cannot solve.
The QR flow is attractive partly because it sidesteps the captcha path entirely — the phone is already authenticated.

## 8. Open questions / insufficient data

The correct current `capabilities` value is unresolved: discord.py-self computes 22525 from its default flags, userdoccers' example shows 1734653, and neither source dates its value.
This must be captured from a live official client before FastCord hardcodes anything, because READY parsing depends on it.

The exact READY field carrying a refreshed token under `AUTH_TOKEN_REFRESH` (bit 8) was not verified.

`launch_signature` in the super properties is generated by a helper in discord.py-self whose algorithm was not read in this pass; its exact derivation (and whether an arbitrary UUID is acceptable) is unverified.

The real `client_build_number` value is not stated here on purpose — the `396858` above comes from a documentation example and is certainly stale by the retrieval date.

Whether `v=2` is still the current remote-auth gateway version, and whether v3 exists, was not independently confirmed beyond the userdoccers page.

Whether Discord actively scores JA3/JA4 TLS fingerprints for user sessions is unverified; only the fact that a maintained client defends against it is established.

The precise REST call sequence the official web client performs between login and the first gateway IDENTIFY was not captured, so "missing typical REST call ordering" is a described risk rather than a specified checklist.

Discord's official documentation contains nothing about any of the user-account behaviour above, so there is no authoritative source to reconcile against — every fact in sections 1 to 5 and the user-specific parts of section 6 rests on reverse engineering that can break at any deploy.

## Sources

- [Remote Authentication (Desktop) — Discord Userdoccers](https://docs.discord.food/remote-authentication/desktop) — retrieved 2026-09-21.
- [Remote Authentication (Overview) — Discord Userdoccers](https://docs.discord.food/remote-authentication/overview) — retrieved 2026-09-21.
- [Remote Authentication (Mobile) — Discord Userdoccers](https://docs.discord.food/remote-authentication/mobile) — retrieved 2026-09-21.
- [Authentication — Discord Userdoccers](https://docs.discord.food/authentication) — retrieved 2026-09-21.
- [Gateway — Discord Userdoccers](https://docs.discord.food/topics/gateway) — retrieved 2026-09-21.
- [Gateway Events — Discord Userdoccers](https://docs.discord.food/topics/gateway-events) — retrieved 2026-09-21.
- [Using the Gateway — Discord Userdoccers](https://docs.discord.food/gateway/using-gateway) — retrieved 2026-09-21.
- [API Reference (client properties, X-Super-Properties) — Discord Userdoccers](https://docs.discord.food/reference) — retrieved 2026-09-21.
- [Experiments — Discord Userdoccers](https://docs.discord.food/topics/experiments) — retrieved 2026-09-21.
- [discord.py-self — `discord/gateway.py` (IDENTIFY construction)](https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py) — retrieved 2026-09-21.
- [discord.py-self — `discord/flags.py` (Capabilities bitfield and default)](https://github.com/dolfies/discord.py-self/blob/master/discord/flags.py) — retrieved 2026-09-21.
- [discord.py-self — `discord/tracking.py` (super properties, build number scraping, TLS impersonation)](https://github.com/dolfies/discord.py-self/blob/master/discord/tracking.py) — retrieved 2026-09-21.
- [Vap0r1ze/discord-remote-auth — remote auth gateway socket wrapper](https://github.com/Vap0r1ze/discord-remote-auth) — retrieved 2026-09-21.
- [Discord Developer Docs — Gateway](https://discord.com/developers/docs/events/gateway) — official, retrieved 2026-09-21.
