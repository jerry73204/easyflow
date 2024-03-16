use crate::ident::Ident;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

/// It describes incoming sinks and outgoing sources to processors of an exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    /// The set of processors which inputs the connection is going into.
    #[serde(rename = "<")]
    pub sink: Option<IndexSet<Ident>>,
    /// The set of processors which outputs the connection is coming from.
    #[serde(rename = ">")]
    pub source: Option<IndexSet<Ident>>,
}

impl Connection {
    /// Iterate over sink names.
    pub fn sink_iter(&self) -> impl Iterator<Item = &Ident> {
        self.sink.as_ref().into_iter().flatten()
    }

    /// Iterate over source names.
    pub fn source_iter(&self) -> impl Iterator<Item = &Ident> {
        self.source.as_ref().into_iter().flatten()
    }

    // pub fn merge(self, other: Self) -> Self {
    //     Self {
    //         sink: merge_set(self.sink, other.sink),
    //         source: merge_set(self.source, other.source),
    //     }
    // }

    // pub fn merge_with(&mut self, other: Self) {
    //     *self = Self {
    //         sink: merge_set(self.sink.take(), other.sink),
    //         source: merge_set(self.source.take(), other.source),
    //     };
    // }
}

// fn merge_set(
//     lhs: Option<IndexSet<Ident>>,
//     rhs: Option<IndexSet<Ident>>,
// ) -> Option<IndexSet<Ident>> {
//     match (lhs, rhs) {
//         (Some(lhs), Some(rhs)) => Some(chain!(lhs, rhs).collect()),
//         (Some(lhs), None) => Some(lhs),
//         (None, Some(rhs)) => Some(rhs),
//         (None, None) => None,
//     }
// }
