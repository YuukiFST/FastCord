# Discord voice protocol and DAVE requirements

Research for FastCord (native Rust/egui client for personal accounts, Windows first).
Question: what does the Discord voice protocol require from a user-account client today, and is it feasible in Rust?

Date of research: 2026-09-21.
Every claim below is followed by the source that owns it; the Sources section at the bottom lists the URLs.

## Bottom line

DAVE end-to-end encryption is mandatory. It has been enforced globally since 2 March 2026 for every non-stage audio/video session type: direct messages, group DMs, guild voice channels and Go Live streams. A client that only implements transport encryption is rejected at the voice gateway with WebSocket close code 4017 and never reaches the UDP stage. This applies to bots and user accounts alike; the community bug reports that followed the enforcement date came from bot libraries (Pycord, DiscordPHP) that had no DAVE support, and they all show the same 4017 close.

Voice is therefore not feasible without DAVE. There is no flag, no legacy voice gateway version, and no "transport only" negotiation that still gets through. The downgrade mechanism described in the DAVE whitepaper (a session falling back to transport-only encryption when a non-supporting participant joins) was a transitional behaviour for the period before enforcement; the whitepaper itself states that once a protocol version is discontinued the voice gateway rejects members that do not support an active or deprecated version.

The good news is that the implementation cost is far lower than "write an MLS stack". A pure-Rust DAVE implementation already exists on crates.io (`davey`, built on OpenMLS), and Songbird 0.6.0 ships DAVE support built on that same crate. So the work for FastCord is integration, not cryptographic engineering.

## What is official and what is reverse-engineered

Officially documented by Discord:

- The voice gateway itself: versions, opcodes, handshake, UDP discovery, encryption modes, Opus expectations, speaking flags, resume rules. This is the public developer documentation at docs.discord.com (it used to live at discord.com/developers/docs/topics/voice-connections and now 301-redirects to docs.discord.com/developers/topics/voice-connections).
- DAVE: the protocol whitepaper at daveprotocol.com and the specification repository github.com/discord/dave-protocol, both published by Discord. Current whitepaper version 1.1.4. The reference C++ implementation, libdave, is open source.
- The enforcement itself: Discord's support article "A/V E2EE Enforcement for Non-Stage Voice Calls" and the accompanying status incident, both dated around 2 March 2026.

Reverse-engineered, not official:

- Everything specific to *user accounts* rather than bots. The reference here is Discord Userdoccers (docs.discord.food), a community reverse-engineering project. It is accurate in practice and widely used, but it carries no guarantee and Discord does not endorse it. Treat its user-account details as "observed behaviour", not as a contract.
- Using a personal account with a custom client is against Discord's Terms of Service regardless of how well it works. That is a product decision for FastCord, not a technical one, but it belongs in the record.

The important consequence: the voice protocol itself is identical for user accounts and bots. Userdoccers states that user accounts follow the same connection flow, with two differences worth noting. A user account can only be connected to one voice channel per session, whereas a bot can be in one per guild. And for a call in a DM or group DM, the *channel ID* is used as the `server_id` in the voice Identify; if the user is the first to join, a call is created and a Call Create gateway event fires.

## Gateway and voice handshake sequence

The sequence, as documented officially:

1. On the main gateway, send Voice State Update (main gateway opcode 4) with `guild_id` (or null for a private call), `channel_id`, `self_mute`, `self_deaf`.
2. Receive the Voice State Update event, which carries `session_id`.
3. Receive the Voice Server Update event, which carries `token`, `guild_id` and `endpoint`.
4. Open a secure WebSocket to `wss://<endpoint>/?v=8`.
5. Send voice Identify (voice opcode 0) with `server_id`, `user_id`, `session_id`, `token` and `max_dave_protocol_version`.
6. Receive Ready (opcode 2): SSRC, IP, port, the list of available encryption modes.
7. Receive Hello (opcode 8): `heartbeat_interval`. Heartbeats are opcode 3, acked with opcode 6.
8. Open the UDP socket to the IP and port from Ready, and perform IP discovery to learn the external address and port.
9. Send Select Protocol (opcode 1) with `protocol: "udp"`, the discovered address and port, and the chosen encryption mode.
10. Receive Session Description (opcode 4): the negotiated mode, the 32-byte `secret_key`, and the DAVE protocol version the gateway selected for the session.
11. Send Speaking (opcode 5) before sending any audio; the speaking mode must be non-zero or the gateway will not forward media.
12. Send RTP packets over UDP.

Voice gateway version 8 is the current recommendation. It adds server message buffering: every server message carries a sequence number, the client tracks the last one seen, and it must send that value as `seq_ack` in both Heartbeat (opcode 3) and Resume (opcode 7). Missed messages are re-delivered on resume. Send `seq_ack: -1` (or omit) if no numbered message has been received yet. Voice gateway versions below 4 were discontinued on 18 November 2024.

Resume flow: on connection loss, open a new WebSocket to the same endpoint, send Resume (opcode 7) with `server_id`, `session_id`, `token`, `seq_ack`, and expect Resumed (opcode 9). If the resume fails the socket closes and the client must run the full connection flow again, starting from a fresh Voice State Update on the main gateway.

### Opcode table

Non-DAVE opcodes: 0 Identify (C to S), 1 Select Protocol (C to S), 2 Ready (S to C), 3 Heartbeat (C to S), 4 Session Description (S to C), 5 Speaking (both), 6 Heartbeat ACK (S to C), 7 Resume (C to S), 8 Hello (S to C), 9 Resumed (S to C). Opcode 13 is Client Disconnect (S to C).

DAVE opcodes, all of them binary-framed rather than JSON: 21 DAVE Protocol Prepare Transition (S to C), 22 DAVE Protocol Execute Transition (S to C), 23 DAVE Protocol Transition Ready (C to S), 24 DAVE Protocol Prepare Epoch (S to C), 25 DAVE MLS External Sender Package (S to C), 26 DAVE MLS Key Package (C to S), 27 DAVE MLS Proposals (S to C), 28 DAVE MLS Commit Welcome (C to S), 29 DAVE MLS Announce Commit Transition (S to C), 30 DAVE MLS Welcome (S to C), 31 DAVE MLS Invalid Commit Welcome (C to S).

Note that the DAVE opcodes are a mixed transport: the voice WebSocket carries both JSON text frames (the classic opcodes) and binary frames (the DAVE ones). A Rust implementation must handle both frame types on the same socket rather than assuming text.

### UDP IP discovery

A single UDP packet, all fields big-endian: Type (uint16, 0x1 for request and 0x2 for the response), Length (uint16, always 70, counting everything after the Length field), SSRC (uint32), Address (64 bytes, null-terminated string), Port (uint16). The client sends the request with its SSRC and empty address/port; the server echoes back with the client's public address and port filled in, which then go into Select Protocol.

## Encryption modes

Two modes are current:

- `aead_aes256_gcm_rtpsize` — preferred when the hardware supports AES (effectively always on modern x86_64, which matters for FastCord's Windows-first target).
- `aead_xchacha20_poly1305_rtpsize` — required, meaning every client must implement it as the guaranteed-available fallback.

Discontinued on 18 November 2024, do not implement: `xsalsa20_poly1305`, `xsalsa20_poly1305_suffix`, `xsalsa20_poly1305_lite`, `xsalsa20_poly1305_lite_rtpsize`, and the non-rtpsize `aead_aes256_gcm`.

For both `_rtpsize` modes the nonce is a 32-bit incremental integer appended to the payload. It must be stripped from the payload before encryption and before decryption, and it is not itself encrypted. The unencrypted header size is determined exactly as in SRTP (RFC 3711), which includes the CSRC list and the optional extension preamble. That is the whole point of the `rtpsize` suffix: the RTP header, CSRCs and extension stay in the clear so that a middlebox can route, and only the payload is sealed.

RTP packet layout: version/flags byte 0x80, payload type 0x78, sequence uint16 BE, timestamp uint32 BE, SSRC uint32 BE, then the encrypted payload.

The `secret_key` from Session Description is 32 bytes and is the transport-encryption key. DAVE sits *inside* this: transport encryption still applies, and DAVE adds a second, end-to-end layer on the media frame before it is placed in the RTP payload. Implementing DAVE does not remove the need to implement transport encryption; it adds to it.

## DAVE

### What it is

DAVE is Discord's end-to-end encryption for audio and video. It applies to DMs, group DMs, guild voice channels (stage channels are excluded) and Go Live streams. It is built on MLS 1.0 (RFC 9420) with ciphersuite 2, `DHKEMP256_AES128GCM_SHA256_P256`, and Basic credentials only. The voice gateway acts as an MLS external sender, which is how Discord injects add/remove proposals for participants joining and leaving without being a group member itself.

Media frames are encrypted with AES-128-GCM using per-sender keys ratcheted from the MLS group's exported secret. For Opus, the entire frame content is encrypted; the codec-aware path that leaves some bytes in the clear exists for video codecs that need parseable headers for packetization. The encrypted frame carries a truncated 8-byte GCM auth tag, a ULEB128 nonce, ULEB128 unencrypted-range pairs, a metadata size byte, and the magic marker `0xFAFA`.

### Negotiation and transitions

The client advertises its highest supported version as `max_dave_protocol_version` in Identify. The gateway picks the session version and returns it in Session Description. Clients are expected to keep backward compatibility with earlier versions.

Group membership changes are coordinated through transitions. The gateway announces one with Prepare Transition (21), the client acknowledges readiness with Transition Ready (23), and the gateway triggers it with Execute Transition (22). A transition ID of 0 means reinitialization and executes immediately rather than being staged. The MLS traffic itself flows over opcodes 25 to 31: the gateway sends its external sender package (25), the client uploads a key package (26), the gateway forwards proposals (27), the client sends back a commit plus welcome (28), and so on.

### Cost estimate for FastCord

Much lower than it looks, because the cryptography is already packaged for Rust.

`davey` (crates.io, repository github.com/Snazzah/davey) is "A Rust implementation of Discord Audio & Video End-to-End Encryption (DAVE) Protocol", first published 4 September 2025, currently at 0.1.4, with roughly 237k total downloads. It is built on OpenMLS 0.8 (`openmls`, `openmls_basic_credential`, `openmls_rust_crypto`) plus the usual RustCrypto pieces (`aes`, `aes-gcm` via `aead`/`ctr`/`ghash`, `p256`, `hmac`, `sha2`, `scrypt`, `subtle`). It is pure Rust, no FFI to libdave, and it also ships Node and Python bindings from the same core via napi and pyo3. Its public API is roughly `DaveSession`, `SigningKeyPair`, `CommitWelcome`, `SessionStatus`, `MediaType`, `Codec`, `ProposalsOperationType`, plus fingerprint helpers for the verification codes Discord shows in the UI.

Crucially, `davey` handles the crypto and MLS state machine but leaves the transport to the caller: FastCord still has to wire the binary voice-gateway opcodes 21 to 31 to the session object. That is the actual work.

Songbird 0.6.0 ("Hoopoe", released 5 April 2026) added DAVE support and depends on `davey` 0.1.2 behind its `driver` feature, alongside `aes-gcm`, `chacha20poly1305`, `opus2`, `discortp` and `rubato`. Songbird is bot-oriented in its gateway integration (serenity/twilight), but its driver is usable standalone, and even if FastCord does not take the dependency, its source is the best worked example of driving `davey` from the voice gateway in Rust.

Realistic estimate, assuming the transport layer is already working: wiring `davey` to the opcode stream is on the order of one focused work item, not a research project. Adopting Songbird's driver wholesale would be less work still but drags in a bot-shaped gateway abstraction that does not fit a user-account client. Writing MLS from scratch would be a multi-month effort and there is no reason to consider it.

## Opus parameters

Officially documented: 48 kHz sample rate, 2 channels (stereo). The payload type in the RTP header is 0x78 (120).

Before stopping transmission, send five frames of silence, the three bytes `0xF8, 0xFF, 0xFE`, to stop the Opus decoder on the other side from interpolating into whatever is transmitted next.

Frame size and bitrate are not pinned down by the official documentation in the material reviewed. In practice every implementation uses 20 ms frames (960 samples per channel at 48 kHz) and this is what the RTP timestamp increment of 960 per packet implies, but treat the exact bitrate target as **insufficient data** from primary sources and settle it empirically against the real client rather than guessing a number here.

## Speaking

Speaking (opcode 5) carries a bitmask, a delay and the SSRC. Flags: Microphone `1 << 0` (normal transmission, shows the speaking indicator), Soundshare `1 << 1` (context audio such as video, no indicator), Priority `1 << 2` (priority speaker, ducks other audio). The mask must be non-zero for media to be forwarded, so push-to-talk means sending Speaking with Microphone set on key-down, transmitting, then sending the silence frames and Speaking with 0 on key-up.

## Open questions

- Exact Opus bitrate and frame duration the official client uses, and whether Discord penalises deviation. Not pinned down by primary sources; resolve empirically.
- Whether a user account gets any different treatment at the voice gateway in practice, beyond the one-channel-per-session limit. Userdoccers says the flow is identical; nobody official confirms it.
- How FastCord should surface the DAVE verification fingerprints. `davey` exposes them, and the official clients display encryption status, but the UX contract is ours to define.
- Whether `davey` 0.1.x is API-stable enough to depend on directly, or whether the integration should be isolated behind a thin FastCord-side trait so the crate can be swapped. Given a 0.1 version number, the trait is the safer call.
- DAVE video and Go Live are out of scope for the first release, but the same MLS group covers them; confirm that ignoring video does not complicate the group state machine.
- Terms of Service exposure for a personal-account client is unresolved and is a product decision, not a technical one.

## Sources

- Discord voice connections documentation: https://docs.discord.com/developers/topics/voice-connections
- DAVE protocol whitepaper (version 1.1.4): https://daveprotocol.com/
- DAVE protocol specification repository: https://github.com/discord/dave-protocol/blob/main/protocol.md
- Discord support, A/V E2EE Enforcement for Non-Stage Voice Calls: https://support.discord.com/hc/en-us/articles/38749827197591-A-V-E2EE-Enforcement-for-Non-Stage-Voice-Calls
- Discord status incident, A/V E2EE enforcement, March 2026: https://isdown.app/status/discord/incidents/545384-a-v-e2ee-enforcement-for-non-stage-voice-calls
- Discord Userdoccers (reverse-engineered), voice connections: https://docs.discord.food/topics/voice-connections
- `davey` crate on crates.io: https://crates.io/crates/davey
- `davey` repository: https://github.com/Snazzah/davey
- `davey` API documentation: https://docs.rs/davey/latest/davey/
- Songbird releases, v0.6.0 "Hoopoe": https://github.com/serenity-rs/songbird/releases
- Songbird `Cargo.toml` (dependency set including `davey`): https://github.com/serenity-rs/songbird/blob/current/Cargo.toml
- Pycord issue 3135, voice closed with 4017 after enforcement: https://github.com/Pycord-Development/pycord/issues/3135
- `oto`, another Rust-native Discord voice transport with v8, RTP and DAVE: https://github.com/rayan6ms/oto
