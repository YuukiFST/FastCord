# Live capture: official web client IDENTIFY capabilities and super properties

Capture date: 2026-09-22.
Source: `https://discord.com/login` loaded in an isolated headless Chrome 153 on Windows 10 19045, not logged in.
Everything below is **OBSERVED** from the served JavaScript bundles and request headers of that session.
None of it is documented by Discord and it can change with any client build.

## Client build number

| Field | Value |
|---|---|
| `client_build_number` | `617136` |
| `RELEASE_CHANNEL` (`window.GLOBAL_ENV`) | `stable` |
| `GATEWAY_ENDPOINT` (`window.GLOBAL_ENV`) | `wss://gateway.discord.gg` |
| `API_VERSION` (`window.GLOBAL_ENV`) | `9` |

The build number is a string literal baked into the bundle (`parseInt("617136",10)`) and copied into super properties.
FastCord must keep refreshing it at runtime, as already locked by the user-account REST research.

## `capabilities` in IDENTIFY

Both gateway paths in the bundle use the same expression:

```js
capabilities: function(e){ let {useChannelObfuscation:t} = e; return t ? 1767421 : 1734653 }
```

- `web.9a6d63589ff469f3.js` (main `GatewaySocket`): `useChannelObfuscation` comes from an experiment lookup keyed `"GatewaySocket"`.
- `fast-connect.4407c918f2e41ed2.js` (early identify before the app bundle loads): `useChannelObfuscation` is true when local storage has `private_channel_obfuscation`.

Decision for FastCord: **`CAPABILITIES = 1734653`** (0x1A77BD).
`1767421` is the same set plus bit 15 (`0x8000`), which enables private-channel obfuscation, an opt-in experiment FastCord must not request.
The 22525 and 55359 values seen in older sources are stale.

## Full IDENTIFY `d` payload shape (main socket)

```js
{
  token: <string>,
  capabilities: 1734653,
  properties: <super properties object, see below>,
  presence: { status, since, activities, afk },
  compress: false,           // compressionHandler.usesLegacyCompression() is false for zlib-stream
  client_state: {            // only when a local cache exists; otherwise { guild_versions: {} }
    guild_versions: {},
    highest_last_message_id, read_state_version, user_guild_settings_version, ...
  },
  qos_token: <opaque, optional>
}
```

The fast-connect path sends a reduced payload: `token`, `capabilities`, `properties` with `is_fast_connect: true` and optional `installation_id`, and `client_state: { guild_versions: {} }`.
FastCord sends the main-socket shape with `client_state: { guild_versions: {} }` and no `qos_token`.

The gateway URL query is `?encoding=json&v=9&compress=zlib-stream`.
A zstd-stream variant exists behind a feature flag and is not needed.

## Super properties (`X-Super-Properties` header, base64 JSON)

Captured verbatim on `GET /api/v9/experiments?with_guild_experiments=true`, browser identity only:

```json
{
  "os": "Windows",
  "browser": "Chrome",
  "device": "",
  "system_locale": "pt-BR",
  "has_client_mods": false,
  "browser_user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/153.0.0.0 Safari/537.36",
  "browser_version": "153.0.0.0",
  "os_version": "10",
  "referrer": "",
  "referring_domain": "",
  "referrer_current": "",
  "referring_domain_current": "",
  "release_channel": "stable",
  "client_build_number": 617136,
  "client_event_source": null,
  "client_launch_id": "<uuid v4, new per launch>",
  "launch_signature": "<uuid v4, new per launch>",
  "client_app_state": "unfocused"
}
```

Companion headers on the same request: `X-Discord-Locale`, `X-Discord-Timezone` (IANA), `X-Debug-Options: bugReporterEnabled`, `X-Context-Properties` (base64 JSON, page-specific).

The desktop-client variant (from the same bundle) replaces `browser` with `"Discord Client"` and adds `client_version`, `os_arch`, `app_arch`, `native_build_number`.
FastCord presents as a browser, so it uses the browser variant above with a real Chrome user agent string (never `HeadlessChrome`).

## Still open: READY sample

A READY payload needs an authenticated gateway session.
This capture was anonymous, so no READY fixture was recorded.
The READY parser fixture ticket depends on a logged-in capture by the product owner (see the map).
