//! Discord snowflake ids: 17-to-20-digit unsigned integers, serialised on the
//! wire as JSON strings.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Discord snowflake id. The inner value is always a 17-to-20-digit number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Snowflake(pub u64);

/// Parsing failures for [`Snowflake::from_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseSnowflakeError;

impl fmt::Display for ParseSnowflakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("snowflake must be 17 to 20 ASCII digits")
    }
}

impl std::error::Error for ParseSnowflakeError {}

impl FromStr for Snowflake {
    type Err = ParseSnowflakeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !(17..=20).contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ParseSnowflakeError);
        }
        s.parse::<u64>()
            .map(Snowflake)
            .map_err(|_| ParseSnowflakeError)
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct IdVisitor;

        impl serde::de::Visitor<'_> for IdVisitor {
            type Value = Snowflake;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a snowflake id as a string or integer")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Snowflake, E> {
                Snowflake::from_str(v).map_err(|_| E::custom("invalid snowflake string"))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Snowflake, E> {
                if v >= 10_000_000_000_000_000 {
                    Ok(Snowflake(v))
                } else {
                    Err(E::custom("snowflake integer below 10^16"))
                }
            }
        }

        d.deserialize_any(IdVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_17_to_20_digit_ids() {
        assert_eq!(
            Snowflake::from_str("10000000000000001").unwrap().0,
            10_000_000_000_000_001
        );
        assert!(Snowflake::from_str("12345678901234567890").is_ok());
    }

    #[test]
    fn rejects_short_non_numeric_and_empty() {
        for bad in ["", "123", "12a45678901234567", " 12345678901234567"] {
            assert!(Snowflake::from_str(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn serialises_as_string() {
        let s = serde_json::to_string(&Snowflake(915059169862453248)).unwrap();
        assert_eq!(s, r#""915059169862453248""#);
    }

    #[test]
    fn deserialises_string_or_integer() {
        let from_str: Snowflake = serde_json::from_str(r#""915059169862453248""#).unwrap();
        let from_int: Snowflake = serde_json::from_str("915059169862453248").unwrap();
        assert_eq!(from_str, from_int);
        assert!(serde_json::from_str::<Snowflake>("123").is_err());
    }
}
