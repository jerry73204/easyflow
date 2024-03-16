use crate::key::Key;
use anyhow::Result;
use itertools::chain;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, fmt, fmt::Display, hash::Hash, str::FromStr, string::ToString};

/// Identifier that consists of ASCII alphanumeric and '-', '_' characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(String);

impl Ident {
    /// Create an identifier from a string.
    pub fn new<'a, S>(name: S) -> Option<Self>
    where
        S: Into<Cow<'a, str>>,
    {
        let name = name.into().into_owned();
        let ok = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c));
        ok.then_some(Self(name))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a [Key] by prepending a prefix.
    pub fn with_prefix<I>(self, prefix: I) -> Key
    where
        I: IntoIterator<Item = Ident>,
    {
        let idents: Vec<_> = chain!(prefix, [self]).collect();
        Key(idents)
    }
}

impl FromStr for Ident {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ident::new(name).ok_or_else(|| format!("invalid name {}", name))
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for Ident {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        let name = String::deserialize(deserializer)?;
        Ident::new(&name).ok_or_else(|| D::Error::custom(format!("invalid name {}", name)))
    }
}
