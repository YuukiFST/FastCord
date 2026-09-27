//! READY decoding: two-pass join of the READY / READY_SUPPLEMENTAL pair (#17).
//!
//! Pass one deserialises `users` into a map; pass two resolves
//! `merged_members` (`user_id`) and `private_channels` (`recipient_ids`)
//! against it. `READY_SUPPLEMENTAL` merges the remaining presences.
//! Decoding runs on the runtime, not the UI thread.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::types::Snowflake;

/// A user known after READY: id plus the display fields v1 needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLite {
    /// User id.
    pub id: Snowflake,
    /// Account username.
    pub username: String,
}

/// A guild with its resolved member list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildLite {
    /// Guild id.
    pub id: Snowflake,
    /// Member user ids, resolved from `merged_members`.
    pub member_ids: Vec<Snowflake>,
}

/// A DM or group DM with resolved recipients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelLite {
    /// Channel id.
    pub id: Snowflake,
    /// Recipient user ids.
    pub recipients: Vec<Snowflake>,
}

/// One channel read-state entry seeding `ReadState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadEntry {
    /// Channel id.
    pub channel_id: Snowflake,
    /// Last read message id, when the channel was ever read.
    pub last_message_id: Option<Snowflake>,
}

/// Everything the UI needs from the READY pair to seed its state slices.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadySnapshot {
    /// Users by id (pass-one map).
    pub users: HashMap<Snowflake, UserLite>,
    /// Guilds with resolved members.
    pub guilds: Vec<GuildLite>,
    /// Private channels with resolved recipients.
    pub private_channels: Vec<ChannelLite>,
    /// Read states seeding `ReadState`.
    pub read_state: Vec<ReadEntry>,
    /// Presence user ids merged from `READY_SUPPLEMENTAL` (additive only).
    pub presences: Vec<Snowflake>,
}

/// Why a READY pair was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyError {
    /// A member or presence references a user absent from `users`.
    UnknownUser(Snowflake),
    /// The payload does not have the expected shape.
    Shape(String),
}

impl fmt::Display for ReadyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUser(id) => write!(f, "READY references unknown user {id}"),
            Self::Shape(why) => write!(f, "READY shape: {why}"),
        }
    }
}

impl std::error::Error for ReadyError {}

#[derive(Debug, Deserialize)]
struct WireUser {
    id: Snowflake,
    #[serde(default)]
    username: String,
}

#[derive(Debug, Deserialize)]
struct WireMember {
    user_id: Snowflake,
}

#[derive(Debug, Deserialize)]
struct WireGuild {
    id: Snowflake,
    #[serde(default)]
    merged_members: Vec<WireMember>,
}

#[derive(Debug, Deserialize)]
struct WireChannel {
    id: Snowflake,
    #[serde(default)]
    recipient_ids: Vec<Snowflake>,
}

#[derive(Debug, Deserialize)]
struct WireRead {
    channel_id: Snowflake,
    last_message_id: Option<Snowflake>,
}

#[derive(Debug, Deserialize)]
struct WireReadState {
    #[serde(default)]
    entries: Vec<WireRead>,
}

#[derive(Debug, Deserialize)]
struct WireReady {
    #[serde(default)]
    users: Vec<WireUser>,
    #[serde(default)]
    guilds: Vec<WireGuild>,
    #[serde(default)]
    private_channels: Vec<WireChannel>,
    #[serde(default)]
    read_state: Option<WireReadState>,
}

#[derive(Debug, Deserialize)]
struct WirePresence {
    user_id: Snowflake,
}

#[derive(Debug, Deserialize, Default)]
struct WireFriends {
    #[serde(default)]
    friends: Vec<WirePresence>,
}

#[derive(Debug, Deserialize, Default)]
struct WireSupplemental {
    #[serde(default)]
    merged_presences: WireFriends,
}

fn parse<T>(raw: &RawValue) -> Result<T, ReadyError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(raw.get()).map_err(|e| ReadyError::Shape(e.to_string()))
}

/// Joins a READY `d` payload with its optional READY_SUPPLEMENTAL `d`.
pub fn join_ready(
    ready: &RawValue,
    supplemental: Option<&RawValue>,
) -> Result<ReadySnapshot, ReadyError> {
    let wire: WireReady = parse(ready)?;
    let mut snap = ReadySnapshot::default();

    for user in wire.users {
        snap.users.insert(
            user.id,
            UserLite {
                id: user.id,
                username: user.username,
            },
        );
    }

    for guild in wire.guilds {
        let mut member_ids = Vec::with_capacity(guild.merged_members.len());
        for member in guild.merged_members {
            if !snap.users.contains_key(&member.user_id) {
                return Err(ReadyError::UnknownUser(member.user_id));
            }
            member_ids.push(member.user_id);
        }
        snap.guilds.push(GuildLite {
            id: guild.id,
            member_ids,
        });
    }

    for channel in wire.private_channels {
        snap.private_channels.push(ChannelLite {
            id: channel.id,
            recipients: channel.recipient_ids,
        });
    }

    if let Some(state) = wire.read_state {
        snap.read_state
            .extend(state.entries.into_iter().map(|e| ReadEntry {
                channel_id: e.channel_id,
                last_message_id: e.last_message_id,
            }));
    }

    if let Some(raw) = supplemental {
        let outer: WireSupplemental = parse(raw)?;
        for presence in outer.merged_presences.friends {
            if !snap.users.contains_key(&presence.user_id) {
                return Err(ReadyError::UnknownUser(presence.user_id));
            }
            snap.presences.push(presence.user_id);
        }
    }

    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::value::to_raw_value;

    fn raw(v: &serde_json::Value) -> Box<RawValue> {
        to_raw_value(v).unwrap()
    }

    #[test]
    fn resolves_members_and_recipients() {
        let ready = serde_json::json!({
            "users": [
                {"id": "100000000000000001", "username": "u1"},
                {"id": "100000000000000002", "username": "u2"}
            ],
            "guilds": [
                {"id": "100000000000000010",
                 "merged_members": [{"user_id": "100000000000000001"}]}
            ],
            "private_channels": [
                {"id": "100000000000000020",
                 "recipient_ids": ["100000000000000002"]}
            ],
            "read_state": {"entries": [
                {"channel_id": "100000000000000020",
                 "last_message_id": "100000000000000030"}
            ]}
        });
        let snap = join_ready(&raw(&ready), None).unwrap();
        assert_eq!(snap.users.len(), 2);
        assert_eq!(snap.guilds[0].member_ids.len(), 1);
        assert_eq!(snap.private_channels[0].recipients.len(), 1);
        assert_eq!(snap.read_state.len(), 1);
    }

    #[test]
    fn unknown_member_user_is_an_error() {
        let ready = serde_json::json!({
            "users": [{"id": "100000000000000001", "username": "u1"}],
            "guilds": [
                {"id": "100000000000000010",
                 "merged_members": [{"user_id": "100000000000000099"}]}
            ]
        });
        assert!(join_ready(&raw(&ready), None).is_err());
    }
}
