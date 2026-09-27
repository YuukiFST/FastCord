//! Discord protocol layer: REST, gateway, models, markdown parser,
//! permissions and rate limiter. No UI, no direct I/O.
//!
//! Crate seams follow `app → {discord, voice, media, storage, platform}` (#22).

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod auth;
pub mod gateway;
pub mod ready;
pub mod types;

#[cfg(any(test, feature = "testing"))]
pub mod testing;
