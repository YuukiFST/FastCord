//! Throwaway spike for FastCord ticket #41.
//!
//! 1. Remote-auth (QR) login per `docs/research/qr-login-and-identify.md`; prints the QR in the terminal.
//! 2. Opens the main gateway with zlib-stream, sends a user-account IDENTIFY per `docs/research/identify-capture.md`.
//! 3. Writes scrubbed READY / READY_SUPPLEMENTAL fixtures plus a capture-meta file to `tests/fixtures/`.
//!
//! The user token lives only in memory (zeroized on drop) and is never printed or written.
//!
//! usage: gateway-capture-spike [--out <dir>]   (default: tests/fixtures)
//!
//! Issue #44: with `FASTCORD_TOKEN` set, remote auth is skipped and the token is used
//! directly (QR login is blocked on the reference phone, see issue #42). The value is
//! shape-checked only; the gateway's close code is the real verdict (4004 = bad token).

use std::{collections::HashMap, path::PathBuf, time::Duration};

use base64::{engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}, Engine};
use flate2::{Decompress, FlushDecompress};
use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use rsa::{pkcs8::EncodePublicKey, Oaep, RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{connect_async, tungstenite::{client::IntoClientRequest, Message}};
use zeroize::Zeroizing;

const CAPABILITIES: u64 = 1_734_653;
const CLIENT_BUILD_NUMBER: u64 = 617_136;
const BROWSER_VERSION: &str = "153.0.0.0";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36";
const REMOTE_AUTH_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?encoding=json&v=9&compress=zlib-stream";
const LOGIN_URL: &str = "https://discord.com/api/v9/users/@me/remote-auth/login";
const ZLIB_SUFFIX: &[u8] = &[0x00, 0x00, 0xff, 0xff];
/// How long to keep reading after READY for READY_SUPPLEMENTAL and the first live events.
const POST_READY_WINDOW: Duration = Duration::from_secs(20);

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = out_dir_from_args();
    std::fs::create_dir_all(&out_dir)?;
    let super_properties = super_properties();

    let token = match token_from_env() {
        Some(token) => {
            eprintln!("[login] using FASTCORD_TOKEN ({} chars), skipping remote auth", token.len());
            token
        }
        None => {
            let token = remote_auth_login(&super_properties).await?;
            eprintln!("[login] token received ({} chars), opening gateway", token.len());
            token
        }
    };

    let capture = gateway_capture(&token, &super_properties).await?;
    drop(token);

    let mut scrubber = Scrubber::new();
    for (name, payload) in [("ready", &capture.ready), ("ready_supplemental", &capture.ready_supplemental)] {
        let Some(payload) = payload else {
            eprintln!("[capture] {name} was NOT received");
            continue;
        };
        let scrubbed = scrubber.scrub(payload.clone());
        let path = out_dir.join(format!("{name}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&scrubbed)?)?;
        eprintln!("[capture] wrote {}", path.display());
    }
    let meta = json!({
        "capture_date": today_utc(),
        "gateway_url": GATEWAY_URL,
        "capabilities": CAPABILITIES,
        "client_build_number": CLIENT_BUILD_NUMBER,
        "browser_version": BROWSER_VERSION,
        "identify_shape": "main-socket: token, capabilities, properties, presence, compress=false, client_state={guild_versions:{}}",
        "hello_heartbeat_interval_ms": capture.heartbeat_interval_ms,
        "close_code": capture.close_code,
        "close_reason": capture.close_reason,
        "events_seen": capture.events_seen,
        "ready_bytes_inflated": capture.ready.as_ref().map(|v| v.to_string().len()),
        "ready_supplemental_bytes_inflated": capture.ready_supplemental.as_ref().map(|v| v.to_string().len()),
        "scrubber_replacements": scrubber.replacements(),
    });
    let meta_path = out_dir.join("capture-meta.json");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    eprintln!("[capture] wrote {}", meta_path.display());
    eprintln!("done");
    Ok(())
}

/// Reads and unsets `FASTCORD_TOKEN`. Trims whitespace and surrounding quotes (pasted values
/// often carry them), then applies the same shape check the client's login screen will use.
fn token_from_env() -> Option<Zeroizing<String>> {
    let raw = Zeroizing::new(std::env::var("FASTCORD_TOKEN").ok()?);
    std::env::remove_var("FASTCORD_TOKEN");
    let trimmed = Zeroizing::new(raw.trim().trim_matches(['"', '\'']).to_owned());
    match token_shape_error(&trimmed) {
        None => Some(trimmed),
        Some(why) => {
            eprintln!("[login] FASTCORD_TOKEN rejected: {why}");
            std::process::exit(2);
        }
    }
}

/// Shape check only, no network. Discord user tokens are three dot-separated base64url
/// segments; the first decodes to the user id (a snowflake). A `mfa.` token is the legacy
/// MFA-account form. Anything else is a paste mistake (a `Bot ` prefix, a password, a URL).
fn token_shape_error(token: &str) -> Option<&'static str> {
    if token.is_empty() { return Some("empty"); }
    if token.starts_with("Bot ") || token.starts_with("Bearer ") { return Some("bot/OAuth prefix; paste the bare user token"); }
    if token.chars().any(|c| c.is_whitespace()) { return Some("contains whitespace"); }
    if token.starts_with("mfa.") { return if token.len() > 20 { None } else { Some("mfa token too short") }; }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 { return Some("expected three dot-separated segments"); }
    let Ok(id_bytes) = URL_SAFE_NO_PAD.decode(parts[0].trim_end_matches('=')) else { return Some("first segment is not base64url"); };
    let Ok(id) = std::str::from_utf8(&id_bytes) else { return Some("first segment does not decode to a user id"); };
    if id.len() < 17 || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) { return Some("first segment does not decode to a user id"); }
    if parts[1].len() < 6 || parts[2].len() < 20 { return Some("segments too short"); }
    None
}

fn out_dir_from_args() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--out" {
            if let Some(v) = args.next() {
                return PathBuf::from(v);
            }
        }
    }
    PathBuf::from("tests/fixtures")
}

// ---------------------------------------------------------------- remote auth

async fn remote_auth_login(super_properties: &Value) -> Result<Zeroizing<String>, Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
    let public_key = RsaPublicKey::from(&private_key);
    let spki_der = public_key.to_public_key_der()?;
    let encoded_public_key = STANDARD.encode(spki_der.as_bytes());
    let expected_fingerprint = URL_SAFE_NO_PAD.encode(Sha256::digest(spki_der.as_bytes()));

    let mut request = REMOTE_AUTH_URL.into_client_request()?;
    request.headers_mut().insert("Origin", "https://discord.com".parse()?);
    request.headers_mut().insert("User-Agent", USER_AGENT.parse()?);
    let (ws, _) = connect_async(request).await?;
    let (mut tx, mut rx) = ws.split();

    let mut heartbeat: Option<tokio::time::Interval> = None;
    let ticket: String;

    loop {
        tokio::select! {
            _ = async { match heartbeat.as_mut() { Some(i) => i.tick().await, None => std::future::pending().await } } => {
                tx.send(Message::text(r#"{"op":"heartbeat"}"#)).await?;
            }
            frame = rx.next() => {
                let Some(frame) = frame else { return Err("remote-auth socket closed before login".into()) };
                let frame = frame?;
                let text = match frame {
                    Message::Text(t) => t.as_str().to_owned(),
                    Message::Close(f) => return Err(format!("remote-auth closed: {f:?}").into()),
                    _ => continue,
                };
                let msg: Value = serde_json::from_str(&text)?;
                let op = msg["op"].as_str().unwrap_or("");
                if op != "heartbeat_ack" { eprintln!("[remote-auth] <- {op}"); }
                match op {
                    "hello" => {
                        let interval_ms = msg["heartbeat_interval"].as_u64().unwrap_or(41_250);
                        let offset = rand::random::<u64>() % interval_ms;
                        let mut i = tokio::time::interval_at(
                            tokio::time::Instant::now() + Duration::from_millis(offset),
                            Duration::from_millis(interval_ms),
                        );
                        i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                        heartbeat = Some(i);
                        eprintln!("[remote-auth] hello: timeout_ms={} heartbeat_interval={}", msg["timeout_ms"], interval_ms);
                        tx.send(Message::text(json!({"op":"init","encoded_public_key":encoded_public_key}).to_string())).await?;
                    }
                    "heartbeat_ack" => {}
                    "nonce_proof" => {
                        let ciphertext = STANDARD.decode(msg["encrypted_nonce"].as_str().unwrap_or(""))?;
                        let nonce = private_key.decrypt(Oaep::new::<Sha256>(), &ciphertext)?;
                        // Confirmed 2026-09-22 against the official web client's own socket traffic (issue #42):
                        // it sends {"op":"nonce_proof","nonce":<base64url of the raw decrypted nonce>}.
                        // Older write-ups (SHA-256 of the nonce under a "proof" key) also pass the handshake
                        // but are not what the official client does; "nonce"+hashed and "proof"+raw get no reply.
                        let proof = URL_SAFE_NO_PAD.encode(&nonce);
                        tx.send(Message::text(json!({"op":"nonce_proof","nonce":proof}).to_string())).await?;
                    }
                    "pending_remote_init" => {
                        let fingerprint = msg["fingerprint"].as_str().unwrap_or("").to_owned();
                        if fingerprint != expected_fingerprint {
                            eprintln!("[remote-auth] WARNING: server fingerprint differs from locally computed SPKI digest");
                        }
                        print_qr(&format!("https://discord.com/ra/{fingerprint}"));
                    }
                    "pending_ticket" => {
                        let ciphertext = STANDARD.decode(msg["encrypted_user_payload"].as_str().unwrap_or(""))?;
                        let payload = Zeroizing::new(String::from_utf8(private_key.decrypt(Oaep::new::<Sha256>(), &ciphertext)?)?);
                        let username = payload.splitn(4, ':').nth(3).unwrap_or("?").to_owned();
                        eprintln!("[remote-auth] scanned by '{username}'. Confirm the login on the phone.");
                    }
                    "pending_login" => {
                        ticket = msg["ticket"].as_str().ok_or("pending_login without ticket")?.to_owned();
                        break;
                    }
                    "cancel" => return Err("login cancelled on the phone".into()),
                    other => eprintln!("[remote-auth] unhandled op {other}"),
                }
            }
        }
    }
    let _ = tx.close().await;

    let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let response: Value = client
        .post(LOGIN_URL)
        .header("X-Super-Properties", STANDARD.encode(super_properties.to_string()))
        .header("X-Discord-Locale", "en-US")
        .json(&json!({ "ticket": ticket }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ciphertext = STANDARD.decode(response["encrypted_token"].as_str().ok_or("no encrypted_token in login response")?)?;
    let token = Zeroizing::new(String::from_utf8(private_key.decrypt(Oaep::new::<Sha256>(), &ciphertext)?)?);
    Ok(token)
}

fn print_qr(url: &str) {
    use qrcode::{render::unicode, QrCode};
    let code = QrCode::new(url.as_bytes()).expect("qr encode");
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    // Issue #42: also write an SVG so the code can be scanned from a browser window
    // when the terminal (or a chat transcript) mangles the half-block rendering.
    let svg = code.render::<qrcode::render::svg::Color>().min_dimensions(400, 400).build();
    match std::fs::write("qr.svg", svg) {
        Ok(()) => eprintln!("[remote-auth] wrote qr.svg"),
        Err(e) => eprintln!("[remote-auth] could not write qr.svg: {e}"),
    }
    eprintln!("\n{image}\n[remote-auth] scan with the Discord mobile app (Settings > Scan QR Code). If the QR does not render, the URL is:\n{url}\n");
}

// ---------------------------------------------------------------- gateway

struct Capture {
    heartbeat_interval_ms: Option<u64>,
    ready: Option<Value>,
    ready_supplemental: Option<Value>,
    events_seen: Vec<String>,
    close_code: Option<u16>,
    close_reason: Option<String>,
}

async fn gateway_capture(token: &str, super_properties: &Value) -> Result<Capture, Box<dyn std::error::Error>> {
    let mut request = GATEWAY_URL.into_client_request()?;
    request.headers_mut().insert("Origin", "https://discord.com".parse()?);
    request.headers_mut().insert("User-Agent", USER_AGENT.parse()?);
    let (ws, _) = connect_async(request).await?;
    let (mut tx, mut rx): (futures_util::stream::SplitSink<Ws, Message>, _) = ws.split();

    let mut inflater = Inflater::new();
    let mut heartbeat: Option<tokio::time::Interval> = None;
    let mut sequence: Option<u64> = None;
    let mut capture = Capture { heartbeat_interval_ms: None, ready: None, ready_supplemental: None, events_seen: vec![], close_code: None, close_reason: None };
    let mut deadline: Option<tokio::time::Instant> = None;

    loop {
        tokio::select! {
            _ = async { match deadline { Some(d) => tokio::time::sleep_until(d).await, None => std::future::pending().await } } => {
                eprintln!("[gateway] post-READY window elapsed, closing");
                let _ = tx.send(Message::Close(None)).await;
                break;
            }
            _ = async { match heartbeat.as_mut() { Some(i) => i.tick().await, None => std::future::pending().await } } => {
                tx.send(Message::text(json!({"op":1,"d":sequence}).to_string())).await?;
            }
            frame = rx.next() => {
                let Some(frame) = frame else { eprintln!("[gateway] socket ended"); break };
                let payload = match frame? {
                    Message::Binary(b) => match inflater.push(&b)? { Some(p) => p, None => continue },
                    Message::Text(t) => t.as_str().to_owned(),
                    Message::Close(f) => {
                        capture.close_code = f.as_ref().map(|f| u16::from(f.code));
                        capture.close_reason = f.as_ref().map(|f| f.reason.as_str().to_owned());
                        eprintln!("[gateway] CLOSE code={:?} reason={:?}", capture.close_code, capture.close_reason);
                        break;
                    }
                    _ => continue,
                };
                let msg: Value = serde_json::from_str(&payload)?;
                if let Some(s) = msg["s"].as_u64() { sequence = Some(s); }
                match msg["op"].as_u64() {
                    Some(10) => {
                        let interval_ms = msg["d"]["heartbeat_interval"].as_u64().unwrap_or(41_250);
                        capture.heartbeat_interval_ms = Some(interval_ms);
                        let offset = rand::random::<u64>() % interval_ms;
                        heartbeat = Some(tokio::time::interval_at(
                            tokio::time::Instant::now() + Duration::from_millis(offset),
                            Duration::from_millis(interval_ms),
                        ));
                        eprintln!("[gateway] HELLO heartbeat_interval={interval_ms}; sending IDENTIFY");
                        let identify = json!({
                            "op": 2,
                            "d": {
                                "token": token,
                                "capabilities": CAPABILITIES,
                                "properties": super_properties,
                                "presence": { "status": "online", "since": 0, "activities": [], "afk": false },
                                "compress": false,
                                "client_state": { "guild_versions": {} }
                            }
                        });
                        tx.send(Message::text(identify.to_string())).await?;
                    }
                    Some(11) => {}
                    Some(1) => { tx.send(Message::text(json!({"op":1,"d":sequence}).to_string())).await?; }
                    Some(9) => {
                        eprintln!("[gateway] INVALID_SESSION (op 9) d={}", msg["d"]);
                        capture.events_seen.push("op9:INVALID_SESSION".into());
                        break;
                    }
                    Some(7) => { capture.events_seen.push("op7:RECONNECT".into()); }
                    Some(0) => {
                        let name = msg["t"].as_str().unwrap_or("?").to_owned();
                        capture.events_seen.push(format!("{name} ({} bytes)", payload.len()));
                        eprintln!("[gateway] DISPATCH {name} ({} bytes)", payload.len());
                        match name.as_str() {
                            "READY" => {
                                capture.ready = Some(msg["d"].clone());
                                deadline = Some(tokio::time::Instant::now() + POST_READY_WINDOW);
                            }
                            "READY_SUPPLEMENTAL" => {
                                capture.ready_supplemental = Some(msg["d"].clone());
                                deadline = Some(tokio::time::Instant::now() + Duration::from_secs(3));
                            }
                            _ => {}
                        }
                    }
                    other => eprintln!("[gateway] op {other:?}"),
                }
            }
        }
    }
    Ok(capture)
}

/// zlib-stream inflater: frames accumulate until the `00 00 ff ff` sync flush suffix.
struct Inflater {
    decompress: Decompress,
    pending: Vec<u8>,
}

impl Inflater {
    fn new() -> Self {
        Self { decompress: Decompress::new(true), pending: Vec::new() }
    }

    fn push(&mut self, frame: &[u8]) -> Result<Option<String>, Box<dyn std::error::Error>> {
        self.pending.extend_from_slice(frame);
        if self.pending.len() < 4 || !self.pending.ends_with(ZLIB_SUFFIX) {
            return Ok(None);
        }
        let mut out = Vec::with_capacity(self.pending.len() * 4);
        let mut consumed = 0usize;
        while consumed < self.pending.len() {
            let before_in = self.decompress.total_in();
            let before_out = self.decompress.total_out();
            self.decompress.decompress_vec(&self.pending[consumed..], &mut out, FlushDecompress::Sync)?;
            consumed += (self.decompress.total_in() - before_in) as usize;
            if self.decompress.total_out() == before_out && consumed < self.pending.len() {
                out.reserve(out.capacity());
            }
        }
        self.pending.clear();
        Ok(Some(String::from_utf8(out)?))
    }
}

// ---------------------------------------------------------------- super properties

fn super_properties() -> Value {
    json!({
        "os": "Windows",
        "browser": "Chrome",
        "device": "",
        "system_locale": "en-US",
        "has_client_mods": false,
        "browser_user_agent": USER_AGENT,
        "browser_version": BROWSER_VERSION,
        "os_version": "10",
        "referrer": "",
        "referring_domain": "",
        "referrer_current": "",
        "referring_domain_current": "",
        "release_channel": "stable",
        "client_build_number": CLIENT_BUILD_NUMBER,
        "client_event_source": null,
        "client_launch_id": uuid_v4(),
        "launch_signature": uuid_v4(),
        "client_app_state": "unfocused"
    })
}

fn uuid_v4() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = |s: &[u8]| s.iter().map(|x| format!("{x:02x}")).collect::<String>();
    format!("{}-{}-{}-{}-{}", h(&b[0..4]), h(&b[4..6]), h(&b[6..8]), h(&b[8..10]), h(&b[10..16]))
}

fn today_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let days = secs / 86_400;
    // civil-from-days (Howard Hinnant), enough for a date stamp without a chrono dependency.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------- scrubber

/// Replaces identifying values with stable placeholders so fixtures keep their structure
/// and cross-references (the same snowflake maps to the same placeholder everywhere).
struct Scrubber {
    snowflakes: HashMap<String, String>,
    strings: HashMap<(String, String), String>,
    token_re: Regex,
    email_re: Regex,
    snowflake_re: Regex,
}

/// Keys whose string values are replaced wholesale with `<key>_<n>`.
const SCRUB_KEYS: &[&str] = &[
    "username", "global_name", "nick", "display_name", "email", "phone", "name", "topic", "description", "bio",
    "content", "avatar", "banner", "icon", "splash", "discovery_splash", "session_id", "analytics_token",
    "auth_session_id_hash", "token", "vanity_url_code", "pronouns", "note", "resume_gateway_url", "rtc_regions",
    "client_launch_id", "launch_signature", "id_hash", "secret", "unique_id", "ip", "address",
];
/// Keys whose whole subtree is dropped (bulk personal data that no parser test needs).
const DROP_KEYS: &[&str] = &["notes", "connected_accounts", "user_settings_proto", "user_settings", "consents", "experiments", "guild_experiments"];

impl Scrubber {
    fn new() -> Self {
        Self {
            snowflakes: HashMap::new(),
            strings: HashMap::new(),
            token_re: Regex::new(r"[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{20,}").unwrap(),
            email_re: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(),
            snowflake_re: Regex::new(r"^\d{17,20}$").unwrap(),
        }
    }

    fn replacements(&self) -> usize {
        self.snowflakes.len() + self.strings.len()
    }

    fn scrub(&mut self, value: Value) -> Value {
        self.walk(value, None)
    }

    fn walk(&mut self, value: Value, key: Option<&str>) -> Value {
        match value {
            Value::Object(map) => {
                let mut out = Map::with_capacity(map.len());
                for (k, v) in map {
                    if DROP_KEYS.contains(&k.as_str()) {
                        out.insert(k, Value::String("<dropped>".into()));
                        continue;
                    }
                    let scrubbed_key = self.map_snowflake(&k).unwrap_or(k);
                    let v = self.walk(v, Some(&scrubbed_key));
                    out.insert(scrubbed_key, v);
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.into_iter().map(|v| self.walk(v, key)).collect()),
            Value::String(s) => Value::String(self.scrub_string(s, key)),
            Value::Number(n) if n.as_u64().map_or(false, |x| x >= 10_000_000_000_000_000) => {
                Value::String(self.map_snowflake(&n.to_string()).unwrap())
            }
            other => other,
        }
    }

    fn scrub_string(&mut self, s: String, key: Option<&str>) -> String {
        if let Some(mapped) = self.map_snowflake(&s) {
            return mapped;
        }
        if let Some(k) = key {
            if SCRUB_KEYS.contains(&k) {
                if s.is_empty() {
                    return s;
                }
                let n = self.strings.len() + 1;
                return self.strings.entry((k.to_owned(), s)).or_insert_with(|| format!("{k}_{n}")).clone();
            }
        }
        let s = self.token_re.replace_all(&s, "<token>").into_owned();
        let s = self.email_re.replace_all(&s, "<email>").into_owned();
        // Snowflakes embedded in URLs or CDN paths.
        let re = Regex::new(r"\d{17,20}").unwrap();
        let mut out = String::with_capacity(s.len());
        let mut last = 0;
        for m in re.find_iter(&s) {
            out.push_str(&s[last..m.start()]);
            out.push_str(&self.map_snowflake(m.as_str()).unwrap());
            last = m.end();
        }
        out.push_str(&s[last..]);
        out
    }

    fn map_snowflake(&mut self, s: &str) -> Option<String> {
        if !self.snowflake_re.is_match(s) {
            return None;
        }
        let n = self.snowflakes.len() + 1;
        Some(self.snowflakes.entry(s.to_owned()).or_insert_with(|| format!("{:018}", 100_000_000_000_000_000u64 + n as u64)).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_shape_accepts_three_segment_user_token() {
        // 19-digit snowflake, base64url, then two opaque segments.
        let id = URL_SAFE_NO_PAD.encode("1234567890123456789");
        let token = format!("{id}.Gh7Xyz.{}", "a".repeat(38));
        assert_eq!(token_shape_error(&token), None);
        assert_eq!(token_shape_error("mfa.".to_owned().as_str()), Some("mfa token too short"));
        assert_eq!(token_shape_error(&format!("mfa.{}", "x".repeat(80))), None);
    }

    #[test]
    fn token_shape_rejects_paste_mistakes() {
        assert!(token_shape_error("").is_some());
        assert!(token_shape_error("Bot abc.def.ghi").is_some());
        assert!(token_shape_error("hunter2").is_some());
        assert!(token_shape_error("https://discord.com/login").is_some());
        assert!(token_shape_error("abc.def.ghi").is_some());
        let id = URL_SAFE_NO_PAD.encode("1234567890123456789");
        assert!(token_shape_error(&format!("{id}.Gh7Xyz.short")).is_some());
    }

    #[test]
    fn scrubber_keeps_cross_references_and_drops_identity() {
        let mut s = Scrubber::new();
        let v = json!({
            "user": { "id": "852892297661906993", "username": "dolfies", "email": "a@b.co" },
            "guilds": [{ "id": "111111111111111111", "owner_id": "852892297661906993", "name": "Secret" }],
            "url": "https://cdn.discordapp.com/avatars/852892297661906993/abc.png",
            "content": "hello",
            "token": "ODUyODkyMjk3NjYxOTA2OTkz.GX5Xdp.22jsdSqEiHLUYEJSsjeq_vJKLpOofd5QMksqw32e"
        });
        let out = s.scrub(v);
        let uid = out["user"]["id"].as_str().unwrap().to_owned();
        assert_eq!(out["guilds"][0]["owner_id"], uid);
        assert_ne!(out["guilds"][0]["id"], uid);
        assert!(out["url"].as_str().unwrap().contains(&uid));
        let placeholder = |v: &Value, key: &str| v.as_str().unwrap().starts_with(&format!("{key}_"));
        assert!(placeholder(&out["user"]["username"], "username"));
        assert!(placeholder(&out["user"]["email"], "email"));
        assert!(placeholder(&out["guilds"][0]["name"], "name"));
        assert!(placeholder(&out["content"], "content"));
        assert!(placeholder(&out["token"], "token"));
        assert!(!out.to_string().contains("dolfies"));
        assert!(!out.to_string().contains("GX5Xdp"));
    }

    #[test]
    fn inflater_reassembles_sync_flushed_stream() {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(br#"{"op":10}"#).unwrap();
        enc.flush().unwrap();
        let first_len = enc.get_ref().len();
        enc.write_all(br#"{"op":11}"#).unwrap();
        enc.flush().unwrap();
        let bytes = enc.get_ref().clone();
        let mut inf = Inflater::new();
        assert_eq!(inf.push(&bytes[..first_len]).unwrap().as_deref(), Some(r#"{"op":10}"#));
        assert_eq!(inf.push(&bytes[first_len..]).unwrap().as_deref(), Some(r#"{"op":11}"#));
    }
}
