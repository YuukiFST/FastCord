//! Gateway wire model: frames, dispatch events, close-code policy, resume
//! backoff and heartbeats (decision #17).
//!
//! The socket frame is [`RawFrame`] (`op`, `s`, `t`, lazy `d`); dispatch
//! events decode lazily into [`GatewayEvent`]. Unknown `t` values count as
//! [`GatewayEvent::Unknown`], never an error. Time and randomness enter
//! [`Backoff`] and [`Heartbeat`] as explicit parameters so tests never sleep.

use serde::Deserialize;
use serde_json::value::RawValue;

/// IDENTIFY capabilities captured from the official client (#32, spike).
pub const CAPABILITIES: u64 = 1_734_653;

/// Main gateway URL: JSON encoding, API v9, zlib-stream compression.
pub const GATEWAY_URL: &str = "wss://gateway.discord.gg/?encoding=json&v=9&compress=zlib-stream";

/// One gateway socket frame. `d` stays lazy: dispatch payloads decode on the
/// runtime only when their handler needs them, never on the UI thread.
#[derive(Debug, Deserialize)]
pub struct RawFrame {
    /// Opcode.
    pub op: u8,
    /// Sequence number (dispatch frames).
    pub s: Option<u64>,
    /// Event name (dispatch frames).
    pub t: Option<String>,
    /// Event payload.
    pub d: Box<RawValue>,
}

/// Decodes one text socket message into a [`RawFrame`].
pub fn decode_frame(text: &str) -> Result<RawFrame, serde_json::Error> {
    serde_json::from_str(text)
}

macro_rules! event_kinds {
    ($($variant:ident => $name:literal),* $(,)?) => {
        /// Dispatch events handled in v1 (#17). Anything else is [`GatewayEvent::Unknown`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum EventKind {
            $(#[doc = concat!("`", $name, "` dispatch event.")] $variant,)*
        }

        impl EventKind {
            /// Maps a wire `t` name to the handled kind, if any.
            pub fn from_name(name: &str) -> Option<Self> {
                Some(match name {
                    $($name => Self::$variant,)*
                    _ => return None,
                })
            }

            /// The wire `t` name of this kind.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }
    };
}

event_kinds! {
    Ready => "READY",
    ReadySupplemental => "READY_SUPPLEMENTAL",
    Resumed => "RESUMED",
    MessageCreate => "MESSAGE_CREATE",
    MessageUpdate => "MESSAGE_UPDATE",
    MessageDelete => "MESSAGE_DELETE",
    MessageDeleteBulk => "MESSAGE_DELETE_BULK",
    MessageReactionAdd => "MESSAGE_REACTION_ADD",
    MessageReactionRemove => "MESSAGE_REACTION_REMOVE",
    MessageReactionRemoveAll => "MESSAGE_REACTION_REMOVE_ALL",
    MessageReactionRemoveEmoji => "MESSAGE_REACTION_REMOVE_EMOJI",
    MessageAck => "MESSAGE_ACK",
    ChannelCreate => "CHANNEL_CREATE",
    ChannelUpdate => "CHANNEL_UPDATE",
    ChannelDelete => "CHANNEL_DELETE",
    ChannelPinsUpdate => "CHANNEL_PINS_UPDATE",
    ChannelUnreadUpdate => "CHANNEL_UNREAD_UPDATE",
    ThreadCreate => "THREAD_CREATE",
    ThreadUpdate => "THREAD_UPDATE",
    ThreadDelete => "THREAD_DELETE",
    ThreadListSync => "THREAD_LIST_SYNC",
    GuildCreate => "GUILD_CREATE",
    GuildUpdate => "GUILD_UPDATE",
    GuildDelete => "GUILD_DELETE",
    GuildRoleCreate => "GUILD_ROLE_CREATE",
    GuildRoleUpdate => "GUILD_ROLE_UPDATE",
    GuildRoleDelete => "GUILD_ROLE_DELETE",
    GuildMemberUpdate => "GUILD_MEMBER_UPDATE",
    GuildMemberListUpdate => "GUILD_MEMBER_LIST_UPDATE",
    GuildMembersChunk => "GUILD_MEMBERS_CHUNK",
    GuildEmojisUpdate => "GUILD_EMOJIS_UPDATE",
    PresenceUpdate => "PRESENCE_UPDATE",
    TypingStart => "TYPING_START",
    UserUpdate => "USER_UPDATE",
    UserGuildSettingsUpdate => "USER_GUILD_SETTINGS_UPDATE",
    UserSettingsProtoUpdate => "USER_SETTINGS_PROTO_UPDATE",
    RelationshipAdd => "RELATIONSHIP_ADD",
    RelationshipRemove => "RELATIONSHIP_REMOVE",
    VoiceStateUpdate => "VOICE_STATE_UPDATE",
    VoiceServerUpdate => "VOICE_SERVER_UPDATE",
    SessionsReplace => "SESSIONS_REPLACE",
}

/// A decoded gateway frame: the typed input of the gateway task.
#[derive(Debug)]
pub enum GatewayEvent {
    /// `op 10`: carries the heartbeat interval in milliseconds.
    Hello {
        /// Heartbeat interval from the HELLO payload.
        heartbeat_interval_ms: u64,
    },
    /// `op 11`: the last heartbeat was acknowledged.
    HeartbeatAck,
    /// `op 7`: the server asks for an immediate resume.
    Reconnect,
    /// `op 9`: session invalid; `resumable` mirrors the `d` boolean.
    InvalidSession {
        /// Whether the session can be resumed.
        resumable: bool,
    },
    /// Known dispatch event with its lazy payload.
    Dispatch {
        /// The handled event kind.
        kind: EventKind,
        /// Raw dispatch payload.
        payload: Box<RawValue>,
    },
    /// Dispatch with an unhandled `t`, or a frame this version ignores.
    /// Counted and logged at `debug`, never an error.
    Unknown {
        /// Frame opcode.
        op: u8,
        /// Event name, when the frame had one.
        t: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct HelloPayload {
    heartbeat_interval: u64,
}

/// Classifies one decoded frame into a [`GatewayEvent`].
pub fn classify(frame: RawFrame) -> GatewayEvent {
    match frame.op {
        0 => match frame.t {
            Some(t) => match EventKind::from_name(&t) {
                Some(kind) => GatewayEvent::Dispatch {
                    kind,
                    payload: frame.d,
                },
                None => GatewayEvent::Unknown { op: 0, t: Some(t) },
            },
            None => GatewayEvent::Unknown { op: 0, t: None },
        },
        7 => GatewayEvent::Reconnect,
        9 => {
            let resumable: bool = serde_json::from_str(frame.d.get()).unwrap_or(false);
            GatewayEvent::InvalidSession { resumable }
        }
        10 => {
            let interval_ms = serde_json::from_str::<HelloPayload>(frame.d.get())
                .map(|h| h.heartbeat_interval)
                .unwrap_or(41_250);
            GatewayEvent::Hello {
                heartbeat_interval_ms: interval_ms,
            }
        }
        11 => GatewayEvent::HeartbeatAck,
        op => GatewayEvent::Unknown { op, t: frame.t },
    }
}

/// What the gateway task does when the socket closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Reconnect to `resume_gateway_url` and send `op 6`.
    Resume,
    /// Wait 1–5 s random, then re-identify (no resume).
    Reidentify,
    /// 4004 (or 4010–4014): stop the task and raise `AuthInvalid`.
    /// The client never re-identifies with a rejected token.
    StopAuthInvalid,
}

/// Maps a gateway close code to the reconnect policy (#17).
pub fn close_action(code: Option<u16>) -> CloseAction {
    match code {
        Some(4004) | Some(4010) | Some(4011) | Some(4012) | Some(4013) | Some(4014) => {
            CloseAction::StopAuthInvalid
        }
        Some(4009) => CloseAction::Reidentify,
        _ => CloseAction::Resume,
    }
}

/// Reconnect backoff: 1 s, 2 s, 4 s … capped at 60 s with ±20 % jitter,
/// reset after a stable minute. Pure: callers pass the clock and the dice.
#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    failures: u32,
    last_change_ms: u64,
}

impl Backoff {
    /// A backoff that has never failed.
    pub fn new() -> Self {
        Self {
            failures: 0,
            last_change_ms: 0,
        }
    }

    /// Next delay in milliseconds. `now_ms` is a monotonic clock,
    /// `rng01` a uniform value in `[0, 1)`.
    pub fn next_delay_ms(&mut self, now_ms: u64, rng01: f64) -> u64 {
        if now_ms.wrapping_sub(self.last_change_ms) >= 60_000 {
            self.failures = 0;
        }
        let shift = self.failures.min(6);
        let base = 1_000u64.saturating_mul(1 << shift).min(60_000);
        let jitter = 0.8 + 0.4 * rng01.clamp(0.0, 1.0);
        self.failures = self.failures.saturating_add(1);
        self.last_change_ms = now_ms;
        (base as f64 * jitter) as u64
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Heartbeat state: interval from HELLO, last sequence, ACK tracking.
/// A beat that goes unacked closes the socket with code 4000 (#17).
#[derive(Debug, Clone, Copy)]
pub struct Heartbeat {
    interval_ms: u64,
    last_seq: Option<u64>,
    awaiting_ack: bool,
}

impl Heartbeat {
    /// Tracks a fresh HELLO interval.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            last_seq: None,
            awaiting_ack: false,
        }
    }

    /// Delay before the first beat, jittered by `rand * interval` (#17).
    pub fn first_delay_ms(&self, rng01: f64) -> u64 {
        (self.interval_ms as f64 * rng01.clamp(0.0, 1.0)) as u64
    }

    /// Records the newest dispatch sequence number.
    pub fn observe_seq(&mut self, seq: u64) {
        self.last_seq = Some(seq);
    }

    /// Emits a beat with the last `s`, or `None` when the previous beat is
    /// still unacked (the caller must close with code 4000 and resume).
    pub fn beat(&mut self) -> Option<Option<u64>> {
        if self.awaiting_ack {
            return None;
        }
        self.awaiting_ack = true;
        Some(self.last_seq)
    }

    /// Records an `op 11` acknowledgement.
    pub fn on_ack(&mut self) {
        self.awaiting_ack = false;
    }

    /// Whether a beat is still waiting for its ACK.
    #[must_use]
    pub fn missed(&self) -> bool {
        self.awaiting_ack
    }
}

/// Builds the `op 2` IDENTIFY payload: token, capabilities, runtime super
/// properties, presence, `compress: false`, `client_state`. Never `intents`.
pub fn build_identify(
    token: &secrecy::SecretString,
    properties: serde_json::Value,
    presence: serde_json::Value,
) -> serde_json::Value {
    use secrecy::ExposeSecret as _;
    serde_json::json!({
        "op": 2,
        "d": {
            "token": token.expose_secret(),
            "capabilities": CAPABILITIES,
            "properties": properties,
            "presence": presence,
            "compress": false,
            "client_state": { "guild_versions": {} },
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_hello_with_interval() {
        let frame = decode_frame(r#"{"op":10,"d":{"heartbeat_interval":41250}}"#).unwrap();
        match classify(frame) {
            GatewayEvent::Hello {
                heartbeat_interval_ms,
            } => assert_eq!(heartbeat_interval_ms, 41250),
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn known_dispatch_decodes_lazily() {
        let frame = decode_frame(
            r#"{"op":0,"s":3,"t":"TYPING_START","d":{"channel_id":"100000000000000001"}}"#,
        )
        .unwrap();
        assert_eq!(frame.s, Some(3));
        match classify(frame) {
            GatewayEvent::Dispatch { kind, .. } => {
                assert_eq!(kind, EventKind::TypingStart);
                assert_eq!(kind.as_str(), "TYPING_START");
            }
            other => panic!("expected Dispatch, got {other:?}"),
        }
    }

    #[test]
    fn unknown_event_name_is_never_an_error() {
        let frame = decode_frame(r#"{"op":0,"s":9,"t":"SOME_FUTURE_EVENT","d":{"x":1}}"#).unwrap();
        match classify(frame) {
            GatewayEvent::Unknown { op, t } => {
                assert_eq!(op, 0);
                assert_eq!(t.as_deref(), Some("SOME_FUTURE_EVENT"));
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn close_code_table() {
        assert_eq!(close_action(Some(4004)), CloseAction::StopAuthInvalid);
        assert_eq!(close_action(Some(4012)), CloseAction::StopAuthInvalid);
        assert_eq!(close_action(Some(4009)), CloseAction::Reidentify);
        assert_eq!(close_action(Some(4000)), CloseAction::Resume);
        assert_eq!(close_action(None), CloseAction::Resume);
    }

    #[test]
    fn backoff_doubles_caps_and_resets() {
        let mut b = Backoff::new();
        let seq: Vec<u64> = (0..8).map(|i| b.next_delay_ms(i * 1_000, 0.5)).collect();
        assert_eq!(seq, [1000, 2000, 4000, 8000, 16000, 32000, 60000, 60000]);
        // Stable for over a minute: back to 1 s.
        assert_eq!(b.next_delay_ms(8_000 + 61_000, 0.5), 1000);
    }

    #[test]
    fn backoff_jitter_stays_within_twenty_percent() {
        let mut low = Backoff::new();
        let mut high = Backoff::new();
        assert_eq!(low.next_delay_ms(0, 0.0), 800);
        assert_eq!(high.next_delay_ms(0, 1.0), 1200);
    }

    #[test]
    fn missed_ack_blocks_the_next_beat() {
        let mut hb = Heartbeat::new(41_250);
        hb.observe_seq(7);
        assert_eq!(hb.beat(), Some(Some(7)));
        assert!(hb.missed());
        assert_eq!(hb.beat(), None);
        hb.on_ack();
        assert!(!hb.missed());
        assert_eq!(hb.beat(), Some(Some(7)));
    }

    #[test]
    fn identify_shape_has_no_intents() {
        use secrecy::SecretString;
        let id = build_identify(
            &SecretString::from("tok".to_owned()),
            serde_json::json!({"os": "windows"}),
            serde_json::json!({"status": "online"}),
        );
        let d = &id["d"];
        assert_eq!(d["capabilities"], CAPABILITIES);
        assert_eq!(d["compress"], false);
        assert!(d.get("intents").is_none());
        assert!(d["client_state"]["guild_versions"].is_object());
    }
}
