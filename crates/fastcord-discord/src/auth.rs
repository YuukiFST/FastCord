//! Login input validation: shape check only, no network round trip.
//!
//! Behaviour is ported from `spikes/gateway-capture` (`token_shape_error`):
//! the gateway close code stays the real verdict (4004 = bad token, #44).
//! The token travels as [` secrecy::SecretString`](https://docs.rs/secrecy)
//! and is exposed only inside [`validate_token_shape`].

use secrecy::{ExposeSecret, SecretString};

/// Why a pasted token was rejected before any network call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenShapeError {
    /// Nothing was pasted.
    Empty,
    /// A `Bot ` or `Bearer ` prefix was pasted with the token.
    Prefixed,
    /// The value contains whitespace (broken paste).
    Whitespace,
    /// Legacy `mfa.` form but too short to be real.
    MfaTooShort,
    /// Not three dot-separated segments.
    SegmentCount,
    /// First segment does not base64url-decode to a user id.
    UserId,
    /// Remaining segments are too short.
    SegmentsTooShort,
}

/// Checks the shape of a pasted user token. Never touches the network.
pub fn validate_token_shape(token: &SecretString) -> Result<(), TokenShapeError> {
    let trimmed = token.expose_secret().trim().trim_matches(['"', '\'']);
    if trimmed.is_empty() {
        return Err(TokenShapeError::Empty);
    }
    if trimmed.starts_with("Bot ") || trimmed.starts_with("Bearer ") {
        return Err(TokenShapeError::Prefixed);
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(TokenShapeError::Whitespace);
    }
    if let Some(rest) = trimmed.strip_prefix("mfa.") {
        if rest.len() + 4 > 20 {
            return Ok(());
        }
        return Err(TokenShapeError::MfaTooShort);
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 3 {
        return Err(TokenShapeError::SegmentCount);
    }
    if !decodes_to_user_id(parts[0]) {
        return Err(TokenShapeError::UserId);
    }
    if parts[1].len() < 6 || parts[2].len() < 20 {
        return Err(TokenShapeError::SegmentsTooShort);
    }
    Ok(())
}

/// The first segment is the base64url (unpadded) user id, a snowflake.
fn decodes_to_user_id(segment: &str) -> bool {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(segment.trim_end_matches('=')) else {
        return false;
    };
    let Ok(id) = std::str::from_utf8(&bytes) else {
        return false;
    };
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape_of(raw: &str) -> Result<(), TokenShapeError> {
        validate_token_shape(&SecretString::from(raw.to_owned()))
    }

    #[test]
    fn accepts_three_segment_and_mfa_tokens() {
        // First segment is base64url("123456789012345678").
        let good = "MTIzNDU2Nzg5MDEyMzQ1Njc4.c2Vnb25kLXNlZ21lbnQtd2hpY2gtaXMtbG9uZy1lbm91Z2g.dGhpcmQtc2VnbWVudC13aGljaC1pcy1sb25nZXItc3RpbGw";
        assert_eq!(shape_of(good), Ok(()));
        assert_eq!(shape_of("mfa.abcdefghijklmnopqrstuvwxyz0123456789ABCD"), Ok(()));
    }

    #[test]
    fn trims_whitespace_and_quotes() {
        let good = "MTIzNDU2Nzg5MDEyMzQ1Njc4.c2Vnb25kLXNlZ21lbnQtd2hpY2gtaXMtbG9uZy1lbm91Z2g.dGhpcmQtc2VnbWVudC13aGljaC1pcy1sb25nZXItc3RpbGw";
        assert_eq!(shape_of(&format!("  \"{good}\"  ")), Ok(()));
    }

    #[test]
    fn rejects_prefixes_whitespace_and_malformed() {
        assert_eq!(shape_of(""), Err(TokenShapeError::Empty));
        assert_eq!(shape_of("Bot abc.def.ghi"), Err(TokenShapeError::Prefixed));
        assert_eq!(shape_of("Bearer abc.def.ghi"), Err(TokenShapeError::Prefixed));
        assert_eq!(shape_of("MTIz.proxy space"), Err(TokenShapeError::Whitespace));
        assert_eq!(shape_of("not a token"), Err(TokenShapeError::SegmentCount));
        assert_eq!(shape_of("mfa.short"), Err(TokenShapeError::MfaTooShort));
        // "aGVsbG8" decodes to "hello", not digits.
        assert_eq!(
            shape_of("aGVsbG8.c2Vnb25k.dGhpcmQtc2VnbWVudC13aGljaC1pcy1sb25n"),
            Err(TokenShapeError::UserId)
        );
        assert_eq!(
            shape_of("MTIzNDU2Nzg5MDEyMzQ1Njc4.short.short"),
            Err(TokenShapeError::SegmentsTooShort)
        );
    }
}
