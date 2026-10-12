//! The identity of one issue, as its tracker spells it.
//!
//! GitHub numbers an issue; Linear names it with a key such as `ENG-123`. The
//! assisted checkout, its branch lock and its ownership marker all name the same
//! issue and must agree on how it is spelled, so one type carries both rather
//! than a second path that disagrees the first time one of them moves.
//!
//! The wire form is untagged — a JSON number reads as `Identity::Number` and a
//! JSON string as `Identity::parse` would read it: a string of digits is a
//! `Number` and any other is a `Key` — so a pointer or ownership marker written
//! before this type existed still reads back as the number it recorded.

use std::fmt;

/// The identity of one issue, as its tracker spells it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(untagged)]
pub enum Identity {
    /// A tracker that numbers its issues — GitHub.
    Number(u64),
    /// A tracker that names its issues — Linear's `ENG-123`.
    Key(String),
}

/// Reading goes through [`Identity::parse`], not through serde's own untagged
/// choice. The two disagreed once and it mattered: a JSON *string* `"12"` fails
/// `Number` and untagged falls to `Key`, while [`Identity::parse`] reads it as
/// `Number(12)`. An MCP argument and the transport flag built from it are the
/// same identity spelled two ways, so the reader must normalise them alike.
impl<'de> serde::Deserialize<'de> for Identity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Read;

        impl serde::de::Visitor<'_> for Read {
            type Value = Identity;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an issue identity: a number, or a key such as `ENG-123`")
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Identity, E> {
                Ok(Identity::Number(value))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Identity, E> {
                u64::try_from(value)
                    .map(Identity::Number)
                    .map_err(|_| E::custom("an issue number cannot be negative"))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Identity, E> {
                Identity::parse(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(Read)
    }
}

impl Identity {
    /// Reads an identity from a tracker flag.
    ///
    /// A run of digits is a number; anything else is a key, upper-cased so
    /// `eng-123` and `ENG-123` are one identity. Blank is refused, because an
    /// empty token names no issue and would silently become one.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("an issue identity cannot be blank".to_owned());
        }
        match text.parse::<u64>() {
            Ok(number) => Ok(Self::Number(number)),
            Err(_) => Ok(Self::Key(text.to_uppercase())),
        }
    }

    /// The number, for the operations only a numbering tracker performs.
    ///
    /// `None` for a key: a Linear issue has no number, and inventing one would
    /// name a different issue.
    pub fn number(&self) -> Option<u64> {
        match self {
            Self::Number(number) => Some(*number),
            Self::Key(_) => None,
        }
    }

    /// Whether this is a key rather than a number.
    pub fn is_key(&self) -> bool {
        matches!(self, Self::Key(_))
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => write!(f, "{number}"),
            Self::Key(key) => write!(f, "{key}"),
        }
    }
}

impl From<u64> for Identity {
    fn from(number: u64) -> Self {
        Self::Number(number)
    }
}

#[cfg(test)]
mod tests {
    use super::Identity;

    #[test]
    fn a_run_of_digits_reads_as_a_number_and_the_rest_as_a_key() {
        assert_eq!(Identity::parse("847"), Ok(Identity::Number(847)));
        assert_eq!(
            Identity::parse("ENG-123"),
            Ok(Identity::Key("ENG-123".to_owned()))
        );
        assert_eq!(Identity::parse("  42  "), Ok(Identity::Number(42)));
    }

    #[test]
    fn a_key_is_one_identity_however_its_team_is_cased() {
        assert_eq!(Identity::parse("eng-123"), Identity::parse("ENG-123"));
        assert_eq!(
            Identity::parse("eng-123"),
            Ok(Identity::Key("ENG-123".to_owned()))
        );
    }

    #[test]
    fn a_blank_identity_is_refused() {
        assert!(Identity::parse("").is_err());
        assert!(Identity::parse("   ").is_err());
    }

    #[test]
    fn a_key_has_no_number() {
        assert_eq!(Identity::Number(7).number(), Some(7));
        assert_eq!(Identity::Key("ENG-1".to_owned()).number(), None);
    }

    #[test]
    fn a_number_written_before_this_type_existed_still_reads_back_as_one() {
        let read: Identity = serde_json::from_str("847").expect("a bare number");
        assert_eq!(read, Identity::Number(847));
        let key: Identity = serde_json::from_str("\"ENG-123\"").expect("a bare string");
        assert_eq!(key, Identity::Key("ENG-123".to_owned()));
    }

    #[test]
    fn a_numeric_string_reads_as_the_same_number_the_transport_parses() {
        // The MCP `assist` argument is a JSON string; the transport flag built
        // from it goes through `Identity::parse`. The pointer is written from
        // the one and the ownership marker from the other, so the two must
        // agree — untagged serde alone read `"12"` as `Key("12")` and made the
        // assisted gate refuse the run's own writes.
        let from_json: Identity = serde_json::from_str("\"12\"").expect("a string argument");
        assert_eq!(from_json, Identity::Number(12));
        assert_eq!(from_json, Identity::parse("12").expect("the flag"));
        let key: Identity = serde_json::from_str("\"ENG-123\"").expect("a key argument");
        assert_eq!(key, Identity::parse("ENG-123").expect("the flag"));
    }

    #[test]
    fn the_display_spelling_is_the_one_a_marker_records() {
        assert_eq!(Identity::Number(847).to_string(), "847");
        assert_eq!(Identity::Key("ENG-123".to_owned()).to_string(), "ENG-123");
    }
}
