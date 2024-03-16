use crate::{Connection, Dir, Ident, Key};
use anyhow::{ensure, Result};
use easyflow_link::Config as Exchange;
use indexmap::{IndexMap, IndexSet};
use itertools::chain;
use serde::{Deserialize, Serialize};
use serde_loader::Json5Path;
use serde_semver::SemverReq;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, SemverReq)]
#[version("0.1.0")]
pub struct Version;

/// The serialized/deserialized graph configuration.
///
/// The graph configurations is valid if
/// - Processor and exchange names are valid identifiers.
/// - Connection identifiers refer to declared exchanges.
/// - Connection sources and sinks are declared processors within the configuration.
///
/// The configuration consists of these components.
/// - `processors`: The processor name list.
/// - `exchanges`: The list of data exchange names and configurations.
/// - `connections`: Defines in/output connections to prorcessors for each exchange.
/// - `modules`: Named external configuration files to be included.
///
/// # Processor Namespace
/// Processor names included from modules are placed in a flat namespace. For example,
/// a module `outer` declares a processor `myproc`. The processor is reference by `myproc`
/// (but not `outer/myproc`).
///
/// # Exchange Namespace
/// The exchange namespace is tree-structured defined by modules.
/// For example, a module named `outer` has an exchange `myexchange`. The exchange
/// is referenced by the key `outer/myexchange`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    /// Format version
    pub version: Version,
    /// Processor name declarations.
    pub processors: IndexSet<Ident>,
    /// Exchange name declarations.
    pub exchanges: IndexMap<Ident, Exchange>,
    /// Connection configurations for local and remote exchanges.
    pub connections: IndexMap<Ident, Connection>,
    /// Outer graph configurations to be included.
    pub modules: Option<IndexMap<Ident, Json5Path<GraphConfig>>>,
}

/// The intermediate working graph data structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphUnchecked {
    pub bindings: IndexMap<PathBuf, Dir>,
    pub processors: IndexSet<Ident>,
    pub exchanges: IndexMap<Key, Exchange>,
    pub connections: IndexMap<Key, Connection>,
}

impl GraphUnchecked {
    /// Merge with the other graph without checking validity.
    pub fn merge_unchecked(mut self, other: Self) -> Self {
        // join processor identifiers
        // it's fine that processors appear more than once
        self.processors.extend(other.processors);

        // combine exchanges and connections
        // assume that exchange identifiers are prepend with a module name
        // so let's join without checks
        self.exchanges.extend(other.exchanges);
        self.connections.extend(other.connections);

        // combine file bindings
        // file paths are unique of there is no cyclic module references
        self.bindings.extend(other.bindings);

        self
    }
}

impl GraphConfig {
    /// The entry point to flatten the modules of a graph configuration.
    pub fn flatten(self) -> Result<GraphUnchecked> {
        self.flatten_recursive(&mut vec![])
    }

    /// The recursive function that flatten the modules of a graph configuration.
    pub fn flatten_recursive(self, prefix: &mut Vec<Ident>) -> Result<GraphUnchecked> {
        // check if connection sinks and sources refer to declared processors
        self.connections
            .values()
            .flat_map(|conn| chain!(conn.sink_iter(), conn.source_iter()))
            .try_for_each(|ident| {
                ensure!(
                    self.processors.contains(ident),
                    "'{}' is not a declared processor",
                    ident
                );
                Ok(())
            })?;

        // check that connections refer to declared exchanges
        self.connections.iter().try_for_each(|(ident, _)| {
            ensure!(
                self.exchanges.contains_key(ident),
                "'{}' is not a declared exchange",
                ident
            );
            Ok(())
        })?;

        // list file bindings for modules
        let bindings: IndexMap<_, _> = self
            .modules
            .iter()
            .flatten()
            .map(|(ident, module)| {
                let path = module.abs_path().to_owned();
                let dir = ident.clone().with_prefix(prefix.iter().cloned()).into_dir();
                (path, dir)
            })
            .collect();

        // prepend prefix to identifiers in current graph
        let this = {
            let prefix_iter = prefix.iter().cloned();

            // add prefix to each exchange name
            let exchanges = self
                .exchanges
                .into_iter()
                .map(|(ident, ex)| {
                    let key = ident.with_prefix(prefix_iter.clone());
                    (key, ex)
                })
                .collect();

            // prepend prefix to each connection
            let connections = self
                .connections
                .into_iter()
                .map(|(ident, conn)| {
                    let key = ident.with_prefix(prefix_iter.clone());
                    (key, conn)
                })
                .collect();

            GraphUnchecked {
                bindings,
                processors: self.processors,
                exchanges,
                connections,
            }
        };

        // recursively merge submodules
        let subgraphs = self.modules.into_iter().flatten().map(|(ident, gconf)| {
            // flatten subgraph
            prefix.push(ident);
            let graph = gconf.take().flatten_recursive(prefix)?;
            prefix.pop();
            Ok(graph)
        });

        let graph = chain!([Ok(this)], subgraphs)
            .reduce(|lhs, rhs| -> Result<_> {
                let lhs = lhs?;
                let rhs = rhs?;
                let merged = lhs.merge_unchecked(rhs);
                Ok(merged)
            })
            .unwrap()?;

        Ok(graph)
    }
}
