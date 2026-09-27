# User-account REST and gateway differences from the bot documentation

Research for [issue #12](https://github.com/YuukiFST/FastCord/issues/12).
Last verified: 2026-09-21.

## How to read the evidence in this file

Discord publishes no documentation for user (non-bot) accounts.
Everything in this file that is specific to a user account is reverse-engineered.
The two source classes used here are:

- **Reverse-engineered, community-maintained**: [Discord Userdoccers](https://docs.discord.food), the successor to the older `discord-unofficial-docs`.
  It is derived from the official client bundle and from observed traffic. It is accurate in practice but carries no stability guarantee, and Discord can break any of it without notice.
- **Observed behaviour in a working implementation**: [`discord.py-self`](https://github.com/dolfies/discord.py-self), a user-account fork of discord.py that has tracked the client for years.
  Where it and Userdoccers agree, confidence is high.

Only a small part of what FastCord needs is in the **official** bot documentation: the rate-limit header names, the multipart upload form, and the generic gateway framing.
Each section below labels which class applies.

## Message search

Reverse-engineered. Bots cannot use search at all, so there is no official counterpart.

The guild endpoint is `GET /guilds/{guild.id}/messages/search` and the channel/DM endpoint is `GET /channels/{channel.id}/messages/search`.
Both take the same query parameters and return the same shape.
The main parameters are `content` (max 1024 chars), `limit` (1-25, default 25), `offset` (max 9975), `max_id` / `min_id` for snowflake bounds, `channel_id`, `author_id`, `author_type` (`user`, `bot`, `webhook`), `mentions`, `mentions_role_id`, `mention_everyone`, `pinned`, `has` (`image`, `video`, `file`, `poll`, ...), `embed_type`, `sort_by` (`timestamp` or `relevance`), `sort_order` (`asc`/`desc`) and `include_nsfw`.
The combination of `limit` capped at 25 and `offset` capped at 9975 means a search result set is hard-capped at roughly 10000 messages.

The response body carries `total_results`, `messages` (an array of arrays: each inner array is the hit plus surrounding context messages), `analytics_id`, `doing_deep_historical_index`, and optionally `channels` and `threads`.
Two behaviours matter for the UI: the endpoint can answer **202 Accepted** when the guild's index is not yet built, in which case the client must retry after the interval given in the body; and `total_results` is approximate while messages are being created or deleted.

There is also a newer tab-based form, `POST /guilds/{guild.id}/messages/search/tabs`, `POST /channels/{channel.id}/messages/search/tabs` and `POST /users/@me/messages/search/tabs`, which runs several filtered queries in parallel in one request.
The `/users/@me` variant is what the client uses for searching across all DMs; it omits results from blocked users.

## Read states, unread and mention counts

Reverse-engineered. Bots have no read state concept.

A read state is a small record holding the last acknowledged entity ID; anything with a higher snowflake is unread, which works because snowflakes are monotonic.
Read states are typed: `CHANNEL` (message unreads), `GUILD_EVENT`, `NOTIFICATION_CENTER`, `GUILD_HOME`, `GUILD_ONBOARDING_QUESTION`, `MESSAGE_REQUESTS`.
Depending on the type the record uses either `last_message_id` or `last_acked_id`, and either `mention_count` or `badge_count` — the pairs are mutually exclusive.
Channel read states additionally carry `flags`, `last_viewed` and `last_pin_timestamp`.

The important design consequence: **the server does not recompute counts for us**.
Userdoccers is explicit that the client has to mirror Discord's own automations locally — increment `mention_count` when an incoming message mentions the user, reset counts to zero when the user sends a message in the channel, and so on.
A FastCord unread badge is therefore client-maintained state seeded from `READY`, not a field we can poll.

Acknowledgement endpoints: `POST /channels/{channel.id}/messages/{message.id}/ack` for a single channel, `POST /read-states/ack-bulk` for several at once, and `DELETE /channels/{channel.id}/messages/ack` to drop a read state.
Non-channel types have their own guild/user endpoints.
Acks made from other sessions arrive over the gateway, so the local counters must also be driven by incoming ack dispatches, not only by our own writes.

## Gateway compression

Partly official (framing), reverse-engineered for what the client actually picks.

Connection query string: `v`, `encoding` (`json` or `etf`) and optional `compress` (`zlib-stream` or `zstd-stream`), e.g. `wss://gateway.discord.gg/?v=9&encoding=json&compress=zlib-stream`.

`zlib-stream` uses a single shared inflate context for the lifetime of the connection; the client buffers websocket frames until the 4-byte `Z_SYNC_FLUSH` suffix `00 00 ff ff` appears, then inflates the accumulated buffer.
`zstd-stream` likewise uses one persistent decompression context, but individual websocket messages do not necessarily end a zstd frame, so the decoder must be a true streaming decoder rather than a one-shot call.

Payload compression (`compress: true` in the identify payload, zlib, applied per-event to selected events only) is the alternative and **cannot be combined** with transport compression.
`discord.py-self` sends `compress` as the negation of whether transport compression is active, with a comment that at least one form of compression is required — so a user-account session is expected to always be compressed one way or the other.

For Rust this means a streaming inflate (`flate2` in zlib streaming mode) keyed to the `00 00 ff ff` boundary, or `zstd`'s streaming decoder.
`zlib-stream` is the safer first target since it is the longest-established path.

## READY, capabilities and lazy guild loading

Reverse-engineered.

A user-account identify differs from a bot identify in two fields that bots never send: `capabilities` (an integer bitfield) and `client_state`.
`intents` is a bot concept; Userdoccers states user accounts do not require intents.
Official clients connect on **v9**, which Userdoccers lists as the client default (v10 exists and is the newer general version; v6 is the deprecated API default).

The capabilities bitfield changes the shape of `READY` substantially:

| Capability | Bit |
|---|---|
| `LAZY_USER_NOTES` | 1 << 0 |
| `NO_AFFINE_USER_IDS` | 1 << 1 |
| `VERSIONED_READ_STATES` | 1 << 2 |
| `VERSIONED_USER_GUILD_SETTINGS` | 1 << 3 |
| `DEDUPE_USER_OBJECTS` | 1 << 4 |
| `PRIORITIZED_READY_PAYLOAD` | 1 << 5 |
| `MULTIPLE_GUILD_EXPERIMENT_POPULATIONS` | 1 << 6 |
| `NON_CHANNEL_READ_STATES` | 1 << 7 |
| `AUTH_TOKEN_REFRESH` | 1 << 8 |
| `USER_SETTINGS_PROTO` | 1 << 9 |
| `CLIENT_STATE_V2` | 1 << 10 |
| `PASSIVE_GUILD_UPDATE` | 1 << 11 |
| `AUTO_CALL_CONNECT` | 1 << 12 |
| `DEBOUNCE_MESSAGE_REACTIONS` | 1 << 13 |
| `PASSIVE_GUILD_UPDATE_V2` | 1 << 14 |
| `CHANNEL_OBFUSCATION` | 1 << 15 |
| `AUTO_LOBBY_CONNECT` | 1 << 16 |

`discord.py-self`'s `Capabilities.default()` enables bits 0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12 and 14, i.e. **55359**.
That is a known-good value for a non-official client as of the current master.

The effects to plan for:

- `DEDUPE_USER_OBJECTS` replaces per-guild `members` with `merged_members`, per-guild `presences` with `merged_presences`, and hoists every user object into a single top-level `users` array; embedded user objects are replaced by bare IDs (`user_id` in members, `recipient_ids` in private channels). Deserialisation must therefore be a two-pass join, not a direct tree decode.
- `VERSIONED_READ_STATES` and `VERSIONED_USER_GUILD_SETTINGS` turn `read_state` and `user_guild_settings` from plain arrays into objects with `entries`, `partial` and `version`. The `version` is what goes back in `client_state` on the next identify so the server can send a delta instead of the whole set.
- `PRIORITIZED_READY_PAYLOAD` splits the initial state: `READY` becomes the minimal critical set and a second `READY_SUPPLEMENTAL` dispatch follows with `guilds` (voice states, activity instances), extra `merged_members`, remaining `merged_presences`, `lazy_private_channels`, `disclose` and `game_invites`.

`READY` for a user account carries far more than the bot version: `v`, `session_id`, `session_type`, `resume_gateway_url`, `static_client_session_id`, `auth_session_id_hash`, `country_code`, `user`, `users`, `guilds`, `private_channels`, `merged_members`, `merged_presences`, `read_state`, `user_guild_settings`, `user_settings_proto` (base64 protobuf), `notification_settings`, `relationships`, `sessions`, `presences`, `analytics_token`, `experiments`, `guild_experiments` and `geo_ordered_rtc_regions`.
Unlike a bot, guilds arrive populated in `READY` rather than trickling in as `GUILD_CREATE`.

`client_state` fields: `guild_versions`, `highest_last_message_id`, `read_state_version`, `user_guild_settings_version`, `user_settings_version`, `private_channels_version`, `api_code_version`, `initial_guild_id`.
`discord.py-self` sends the minimal form with only an empty `guild_versions` map, which is valid and simply forgoes delta optimisation.
FastCord can start there.

### Lazy loading and member lists

`READY` deliberately does not contain guild member lists. They are fetched by subscribing per channel.

Opcode **14, Guild Subscriptions**, is the send-only opcode for this, and Userdoccers now marks it **deprecated** in favour of opcode **37, Guild Subscriptions Bulk**, which carries a `subscriptions` object covering several guilds in one payload.
`discord.py-self` implements both (`GUILD_SUBSCRIBE` = 14, `BULK_GUILD_SUBSCRIBE` = 37) and still uses op 14 for single-guild updates.

The op 14 payload shape, from the unofficial lazy-guilds documentation and corroborated by the field list `discord.py-self` sends (typing, threads, activities, members, channels, thread_member_lists):

```json
{
  "op": 14,
  "d": {
    "guild_id": "...",
    "channels": { "<channel_id>": [[0, 99], [100, 199]] },
    "members": [],
    "activities": true,
    "typing": true,
    "threads": false
  }
}
```

`channels` maps a channel ID to member-list ranges.
Ranges are 100 wide and aligned: `[100*n, 100*n+99]`.
The client always requests `[0, 99]` and may request two further ranges in the same call.

The server answers with `GUILD_MEMBER_LIST_UPDATE`, carrying `guild_id`, `id`, `member_count`, `online_count`, `groups` and `ops`.
`id` is the member-list identity: literally `everyone` when the channel has no permission overwrites affecting visibility, otherwise a hash derived from those overwrites — channels sharing a permission shape share a list.
`groups` are the hoisted-role buckets, each `{id, count}` where `id` is a role snowflake or the literal `online` / `offline`.
`ops` entries are `SYNC` (a range's full contents, as interleaved group headers and member entries), `INSERT`, `UPDATE`, `DELETE` (incremental, carrying an `index`) and `INVALIDATE` (the requested range is outside the real list, so back off).
Members are ordered alphanumerically by nickname, falling back to username.

Supporting opcodes worth knowing: **8** Request Guild Members (same as bots, chunked at 1000 per `GUILD_MEMBERS_CHUNK`), **13** Call Connect, **28** Request Forum Unreads, **34** Request Last Messages (`guild_id` plus up to 100 `channel_ids`), **38** Guild Channels Resync, **39** Request Channel Member Count, **40** QoS Heartbeat, **41** Update Time Spent Session ID.

## Rate limits

Header names are officially documented and identical for user tokens.
The user-token differences are reverse-engineered.

Headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`, `X-RateLimit-Reset-After`, `X-RateLimit-Bucket`, and on 429 additionally `Retry-After`, `X-RateLimit-Global` and `X-RateLimit-Scope` (`user`, `global`, `shared`).
A 429 body has `message`, `retry_after` (float seconds), `global` (bool) and optionally `code`.

The one difference that actually changes our client design: **user tokens typically receive only `Retry-After`, `X-RateLimit-Global` and `X-RateLimit-Scope`** — the per-bucket headers are usually absent.
A bot library's proactive bucket-tracking limiter therefore has nothing to track on a user token.
FastCord's limiter has to be reactive: queue per route, obey the 429 `retry_after`, and keep a conservative client-side ceiling.

The global ceiling is 50 requests per second, the same figure as for bots (the documented bot escalation path to 1200/s does not apply to users).
Separately, Cloudflare bans an IP for 24 hours after 10000 invalid requests (401/403/429) in 10 minutes; 429s with `X-RateLimit-Scope: shared` are excluded from that count.
This makes an unauthenticated-retry bug on a user token far more dangerous than on a bot: a token that has gone invalid and is retried in a loop will get the IP banned.

## Attachment upload

The multipart path is officially documented; the cloud path is reverse-engineered and is what the real client uses.

Legacy path: `multipart/form-data` with files as `files[n]`, a `payload_json` field holding the JSON body, and an `attachments` array in that JSON describing each file; embeds reference them as `attachment://filename`.

Cloud path, what the desktop client does:

1. `POST /channels/{channel.id}/attachments` with a `files` array; each entry has `filename` (max 1024 chars), `file_size` in bytes, an optional caller-chosen `id` for correlation, and optionally `is_clip` and `original_content_type`.
2. The response is an array of cloud attachment objects with `upload_url` (a signed Google Cloud Storage URL), `upload_filename`, and the echoed `id`.
3. `PUT` the raw file bytes to `upload_url`. No Discord auth header belongs on this request.
4. Create the message referencing `uploaded_filename` in its `attachments` entries instead of resending bytes.
5. If the upload is abandoned, `DELETE /attachments/{upload_filename}` cleans it out of the bucket.

Limits: 500 MiB per file to GCS (100 MiB for clipped stream recordings), but the user's own attachment size limit still gates what the message-create call will accept.
The practical gain for FastCord is that upload progress and cancellation become trivial, because step 3 is a plain HTTP PUT we fully control.

## Request headers and super properties

Reverse-engineered, except `Authorization` and `User-Agent`.

`Authorization` on a user token is the bare token, **no prefix** (bots use `Bot `, OAuth2 uses `Bearer `).

`X-Super-Properties` is a base64-encoded JSON object sent on every HTTP request and also as `properties` in the gateway identify.
Userdoccers documents the keys: `os`, `browser`, `device`, `system_locale`, `has_client_mods`, `browser_user_agent`, `browser_version`, `os_version`, `referrer`, `referring_domain`, `referrer_current`, `referring_domain_current`, `release_channel`, `client_build_number`, `client_event_source`, `client_app_state`, `native_build_number`, `client_launch_id`, `client_heartbeat_session_id`, `design_id`.
The documented desktop example also carries `client_version`, `os_arch`, `app_arch`, `os_sdk_version` and `launch_signature`:

```json
{
  "os": "Windows",
  "browser": "Discord Client",
  "release_channel": "stable",
  "client_version": "1.0.328",
  "os_version": "10.0.26100",
  "os_arch": "x64",
  "app_arch": "x64",
  "system_locale": "en-US",
  "has_client_mods": false,
  "client_launch_id": "466a709c-1d91-442d-a35f-34e8c834736e",
  "launch_signature": "477bea01-90cb-422d-9a38-aaa66ed3e25e",
  "browser_user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.328 Chrome/134.0.6998.179 Electron/35.1.5 Safari/537.36",
  "browser_version": "35.1.5",
  "os_sdk_version": "26100",
  "client_build_number": 397417,
  "native_build_number": 63309,
  "client_event_source": null,
  "client_heartbeat_session_id": "b789aacc-2579-489a-9dc8-2aa440519ae6"
}
```

`client_build_number` is the field that goes stale: it tracks the live desktop build, and Discord has rejected or degraded requests carrying obviously old values.
Whatever FastCord ships must be refreshable at runtime rather than compiled in as a constant.
`User-Agent` on the HTTP request should match `browser_user_agent` for the identity to be self-consistent.

Other documented headers: `X-Discord-Locale` (determines the locale the API answers in) and `X-Debug-Options` (comma-delimited, accepts `canary`, `trace`).

### Insufficient data

Userdoccers does not document `X-Discord-Timezone`, `X-Context-Properties`, `X-Track` or `X-Fingerprint`, and gives no fingerprinting description beyond `launch_signature` in the client properties.
`X-Context-Properties` in particular is known to be sent by the real client on actions such as joining a guild or opening a DM, but its per-action values are not documented in any primary source this research found.
Treat it as unknown and omit it until an action actually fails without it.

## Sources

- Discord Userdoccers, Messages (search endpoints): https://docs.discord.food/resources/message#search-messages
- Discord Userdoccers, Read State: https://docs.discord.food/topics/read-state
- Discord Userdoccers, Gateway (connection, compression): https://docs.discord.food/topics/gateway
- Discord Userdoccers, Gateway Events (READY, READY_SUPPLEMENTAL, identify, client state): https://docs.discord.food/gateway/gateway-events
- Discord Userdoccers, Opcodes and Close Codes: https://docs.discord.food/gateway/opcodes-and-close-codes
- Discord Userdoccers, Using Gateway (capabilities bitfield): https://docs.discord.food/gateway/using-gateway
- Discord Userdoccers, Rate Limits: https://docs.discord.food/topics/rate-limits
- Discord Userdoccers, Cloud Uploads: https://docs.discord.food/topics/cloud-uploads
- Discord Userdoccers, API Reference (versioning, authentication, headers, client properties, multipart): https://docs.discord.food/reference
- Unofficial Discord API Docs, Lazy Guilds (op 14, GUILD_MEMBER_LIST_UPDATE): https://arandomnewaccount.gitlab.io/discord-unofficial-docs/lazy_guilds.html
- discord.py-self, `discord/gateway.py` (identify construction, compression choice, op 14/37 usage): https://github.com/dolfies/discord.py-self/blob/master/discord/gateway.py
- discord.py-self, `discord/flags.py` (`Capabilities`, `Capabilities.default()`): https://github.com/dolfies/discord.py-self/blob/master/discord/flags.py
- Discord official developer documentation, Rate Limits: https://discord.com/developers/docs/topics/rate-limits
