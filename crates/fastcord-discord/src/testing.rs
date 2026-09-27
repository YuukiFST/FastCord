//! Named fakes for tests (decision #24): scripted doubles behind
//! `#[cfg(any(test, feature = "testing"))]`, no network, no sleeps.

use crate::gateway::{classify, decode_frame, EventKind, GatewayEvent};

/// One scripted transport step.
#[derive(Debug, Clone, Copy)]
pub enum FakeStep {
    /// A raw socket text message.
    Frame(&'static str),
    /// A socket close with an optional close code.
    Close(Option<u16>),
}

/// What the replayed script produced, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    /// `op 10`, with the heartbeat interval in milliseconds.
    Hello(u64),
    /// A known dispatch event kind.
    Dispatched(EventKind),
    /// The socket closed.
    Closed(Option<u16>),
}

/// Replays a scripted `Vec<FakeStep>`: frames decode through the real
/// [`classify`] path, closes surface as [`TransportEvent::Closed`].
/// Non-dispatch control frames (ACKs, reconnects) are consumed silently,
/// like the gateway task consumes them.
#[derive(Debug)]
pub struct FakeGateway {
    script: Vec<FakeStep>,
    pos: usize,
}

impl FakeGateway {
    /// Builds a replay gateway over the given script.
    pub fn new(script: Vec<FakeStep>) -> Self {
        Self { script, pos: 0 }
    }

    /// Returns the next decoded event, or `None` when the script ends.
    /// Panics on malformed script frames: a broken script is a test bug.
    pub fn next(&mut self) -> Option<TransportEvent> {
        loop {
            let step = self.script.get(self.pos)?;
            self.pos += 1;
            match step {
                FakeStep::Close(code) => return Some(TransportEvent::Closed(*code)),
                FakeStep::Frame(text) => {
                    let frame = decode_frame(text).expect("script frame decodes");
                    match classify(frame) {
                        GatewayEvent::Hello {
                            heartbeat_interval_ms,
                        } => return Some(TransportEvent::Hello(heartbeat_interval_ms)),
                        GatewayEvent::Dispatch { kind, .. } => {
                            return Some(TransportEvent::Dispatched(kind));
                        }
                        _ => continue,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::{close_action, CloseAction};

    #[test]
    fn replays_hello_ready_and_close() {
        let mut gw = FakeGateway::new(vec![
            FakeStep::Frame(r#"{"op":10,"d":{"heartbeat_interval":41250}}"#),
            FakeStep::Frame(r#"{"op":11,"d":null}"#),
            FakeStep::Frame(r#"{"op":0,"s":1,"t":"READY","d":{}}"#),
            FakeStep::Frame(
                r#"{"op":0,"s":2,"t":"READY_SUPPLEMENTAL","d":{"friends":[]}}"#,
            ),
            FakeStep::Close(None),
        ]);
        assert_eq!(gw.next(), Some(TransportEvent::Hello(41250)));
        assert_eq!(
            gw.next(),
            Some(TransportEvent::Dispatched(EventKind::Ready))
        );
        assert_eq!(
            gw.next(),
            Some(TransportEvent::Dispatched(EventKind::ReadySupplemental))
        );
        assert_eq!(gw.next(), Some(TransportEvent::Closed(None)));
        assert_eq!(gw.next(), None);
        assert_eq!(close_action(None), CloseAction::Resume);
    }
}
