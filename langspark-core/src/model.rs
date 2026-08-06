//! Shared domain types
//!
//! Newtype wrappers around `String` for the core vocabulary concepts, so
//! function signatures document intent (a `Word` can't be passed where a
//! `Meaning` is expected) instead of everything being a bare `String`.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_newtype {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }
    };
}

string_newtype!(
    /// A word or phrase in its target-language written form (e.g. "受け取る", "recibir").
    Word
);
string_newtype!(
    /// The phonetic reading of a `Word` (e.g. "うけとる" or a Spanish phonetic guide).
    Reading
);
string_newtype!(
    /// A gloss/translation of a `Word` (e.g. "to receive").
    Meaning
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newtype_conversions() {
        let word: Word = "受け取る".into();
        assert_eq!(word.as_str(), "受け取る");
        assert_eq!(word.to_string(), "受け取る");
        assert_eq!(&*word, "受け取る");
    }

    #[test]
    fn test_newtype_serde_roundtrip() {
        let reading = Reading::from("うけとる");
        let json = serde_json::to_string(&reading).unwrap();
        let back: Reading = serde_json::from_str(&json).unwrap();
        assert_eq!(reading, back);
    }
}
