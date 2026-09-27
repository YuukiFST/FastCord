//! Gateway replay test: joins the (currently synthetic) READY pair from
//! `tests/fixtures/gateway/` through the real [`join_ready`](fastcord_discord::ready::join_ready)
//! path. Issue #45 replaces the fixtures with the real capture; the test
//! stays the same.

#![allow(clippy::unwrap_used)]

use fastcord_discord::ready::join_ready;

fn raw(value: &serde_json::Value) -> Box<serde_json::value::RawValue> {
    serde_json::value::to_raw_value(value).unwrap()
}

#[test]
fn joins_users_members_and_dm_recipients() {
    let ready: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/gateway/ready.json").unwrap(),
    )
    .unwrap();
    let supplemental: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/gateway/ready_supplemental.json").unwrap(),
    )
    .unwrap();
    let snap = join_ready(&raw(&ready), Some(&raw(&supplemental))).unwrap();
    assert_eq!(snap.users.len(), 2);
    assert_eq!(snap.guilds.len(), 1);
    assert_eq!(snap.guilds[0].member_ids.len(), 1);
    assert_eq!(snap.private_channels.len(), 1);
    assert_eq!(snap.private_channels[0].recipients.len(), 1);
    assert_eq!(snap.read_state.len(), 1);
    assert_eq!(snap.presences.len(), 2);
}

#[test]
fn unknown_member_user_is_an_error() {
    let mut ready: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/gateway/ready.json").unwrap(),
    )
    .unwrap();
    ready["guilds"][0]["merged_members"][0]["user_id"] = serde_json::json!("100000000000000099");
    assert!(join_ready(&raw(&ready), None).is_err());
}
