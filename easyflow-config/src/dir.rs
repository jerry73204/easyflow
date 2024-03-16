use crate::ident::Ident;
use anyhow::Result;
use itertools::Itertools as _;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, fmt::Display, hash::Hash, str::FromStr};

/// Represent sequence of identifers that could be empty.
///
/// It can be parsed from an empty string, an identifier string `myname` or a path notation `mydir/myname`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dir(pub(super) Vec<Ident>);

impl Dir {
    /// Create from an identifier sequence.
    pub fn new(idents: Vec<Ident>) -> Self {
        Self(idents)
    }
}

impl Serialize for Dir {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let idents: Vec<_> = self.0.iter().map(|ident| ident.as_str()).collect();
        let text = idents.join("/");
        text.serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Dir {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        let text = String::deserialize(deserializer)?;
        let idents: Vec<Ident> = text
            .split('/')
            .map(Ident::from_str)
            .try_collect()
            .map_err(D::Error::custom)?;

        Ok(Dir::new(idents))
    }
}

impl Display for Dir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let idents: Vec<_> = self.0.iter().map(|ident| ident.as_str()).collect();
        let text = idents.join("/");
        text.fmt(f)
    }
}
