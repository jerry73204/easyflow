use crate::{dir::Dir, ident::Ident};
use anyhow::Result;
use itertools::{chain, Itertools as _};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, fmt::Display, hash::Hash, str::FromStr, string::ToString};

/// Represent non-empty sequence of identifers.
///
/// It can be parsed from an identifier string `myname` or a path notation `mydir/myname`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key(pub(super) Vec<Ident>);

impl Key {
    /// Create from an identifier sequence.
    ///
    /// The sequence must be non-empty. Otherwise it returns `None`.
    pub fn new(idents: Vec<Ident>) -> Option<Self> {
        let ok = !idents.is_empty();
        ok.then_some(Self(idents))
    }

    /// Convert to [`Dir`](crate::dir::Dir).
    pub fn into_dir(self) -> Dir {
        Dir(self.0)
    }

    /// Obtain the last identifier in the key.
    pub fn last_ident(&self) -> &Ident {
        self.0.last().as_ref().unwrap()
    }

    /// Return the identifier if the key consists exactly one identifier.
    pub fn as_ident(&self) -> Option<&Ident> {
        if self.0.len() == 1 {
            let name = &self.0[0];
            Some(name)
        } else {
            None
        }
    }

    /// Return true if the key consists exactly one identifier.
    pub fn is_ident(&self) -> bool {
        self.0.len() == 1
    }

    /// Prepend an identifier sequence and returns a new key.
    pub fn prepend<I>(&mut self, idents: I)
    where
        I: IntoIterator<Item = Ident>,
    {
        self.0 = chain!(idents, self.0.drain(..)).collect();
    }
}

impl FromStr for Key {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let idents: Vec<Ident> = text.split('/').map(Ident::from_str).try_collect()?;
        let key = Self::new(idents).ok_or_else(|| String::from("key must not be empty"))?;
        Ok(key)
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let idents: Vec<_> = self.0.iter().map(|ident| ident.as_str()).collect();
        let text = idents.join("/");
        text.fmt(f)
    }
}

impl Serialize for Key {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // let idents: Vec<_> = self.0.iter().map(|ident| ident.as_str()).collect();
        // let text = idents.join("/");
        self.to_string().serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        let text = String::deserialize(deserializer)?;
        let idents: Result<Vec<Ident>, _> = text.split('/').map(Ident::from_str).collect();

        let idents = idents.map_err(D::Error::custom)?;
        if idents.is_empty() {
            return Err(D::Error::custom("key must not be empty"));
        }

        Ok(Key::new(idents).unwrap())
    }
}
