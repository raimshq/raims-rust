use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A request field that is omitted, explicitly null, or set to a value.
#[derive(Clone, PartialEq, Eq)]
pub enum Optional<T> {
    /// Omit the field from the JSON object.
    Unset,
    /// Send JSON null when the endpoint accepts it.
    Null,
    /// Send a value, including empty strings, lists, and maps.
    Value(T),
}

// Preserve the explicit, bound-free implementation recorded by the public API baseline.
#[allow(clippy::derivable_impls)]
impl<T> Default for Optional<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<T> Optional<T> {
    /// Whether this field should be omitted.
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    /// Borrow a value without changing omission or null.
    pub fn as_ref(&self) -> Optional<&T> {
        match self {
            Self::Unset => Optional::Unset,
            Self::Null => Optional::Null,
            Self::Value(value) => Optional::Value(value),
        }
    }
}

impl<T> From<T> for Optional<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T> fmt::Debug for Optional<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unset => "Unset",
            Self::Null => "Null",
            Self::Value(_) => "Value([redacted])",
        })
    }
}

impl<T: Serialize> Serialize for Optional<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Null => serializer.serialize_none(),
            Self::Unset => Err(serde::ser::Error::custom(
                "Unset fields must use skip_serializing_if = Optional::is_unset.",
            )),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Optional<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}
