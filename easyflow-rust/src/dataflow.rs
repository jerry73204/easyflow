use crate::error::Error;
use anyhow::Result;
use derivative::Derivative;
use easyflow_config::{
    Connection, Dir, GraphConfig, GraphUnchecked, Ident, IntoIdent, IntoKey, Key,
};
use easyflow_link::Config as Exchange;
use indexmap::{IndexMap, IndexSet};
use itertools::{chain, Itertools};
use ownref::{ArcOwnedC, ArcRefC};
use serde::{Deserialize, Serialize};
use serde_loader::Json5Path;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufWriter},
    path::{Path, PathBuf},
};

type ARef<T> = ArcRefC<'static, GraphUnchecked, T>;
type AOwned<T> = ArcOwnedC<'static, GraphUnchecked, T>;

// TODO: serialize of graph to config..
/// The graph data structure that describes the dataflow.
#[derive(Debug, Clone, Serialize, Deserialize, Derivative)]
#[serde(try_from = "GraphConfig", into = "GraphConfig")]
#[derivative(PartialEq, Eq)]
pub struct Dataflow {
    #[derivative(PartialEq = "ignore")]
    bindings: ARef<IndexMap<PathBuf, Dir>>,
    processors: ARef<IndexSet<Ident>>,
    exchanges: ARef<IndexMap<Key, Exchange>>,
    adj_exchange: ARef<IndexMap<Key, Connection>>,
    adj_processor: HashMap<ARef<Ident>, ProcInOut>,
    #[derivative(PartialEq = "ignore")]
    config: GraphConfig,
}

/// The input and output exchanges of a processor.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcInOut {
    /// Connections to exchange sources.
    pub inputs: Option<HashSet<ARef<Key>>>,
    /// Connections to exchange sinks.
    pub outputs: Option<HashSet<ARef<Key>>>,
}

impl Dataflow {
    /// Open a dataflow configuration file in JSON5 format.
    pub fn open<F>(file: F) -> Result<Self>
    where
        F: AsRef<Path>,
    {
        Ok(Json5Path::open_and_take(file)?)
    }

    /// Construct a graph from configuration data.
    pub fn from_config(config: GraphConfig) -> Result<Self> {
        let base = ARef::new(config.clone().flatten()?);
        let bindings = base.clone().map(|g| &g.bindings);
        let processors = base.clone().map(|g| &g.processors);
        let exchanges = base.clone().map(|g| &g.exchanges);
        let adj_exchange = base.clone().map(|g| &g.connections);

        let mut proc_to_sinks: HashMap<ARef<Ident>, HashSet<ARef<Key>>> =
            ARef::into_arc_owned(adj_exchange.clone())
                .flatten()
                .flat_map(|pair| {
                    let ex_key: AOwned<&Key> = pair.clone().map(|(key, _)| key);
                    let ex_key: ARef<Key> = AOwned::into_arc_ref(ex_key);
                    let sinks = pair
                        .flat_map(|(_, conn)| conn.sink_iter())
                        .map(AOwned::into_arc_ref);
                    sinks.map(move |sink| (sink, ex_key.clone()))
                })
                .into_grouping_map()
                .collect();
        let mut proc_to_srcs: HashMap<ARef<Ident>, HashSet<ARef<Key>>> =
            ARef::into_arc_owned(adj_exchange.clone())
                .flatten()
                .flat_map(|pair| {
                    let ex_key: AOwned<&Key> = pair.clone().map(|(key, _)| key);
                    let ex_key: ARef<Key> = AOwned::into_arc_ref(ex_key);
                    let sources = pair
                        .flat_map(|(_, conn)| conn.source_iter())
                        .map(AOwned::into_arc_ref);
                    sources.map(move |src| (src, ex_key.clone()))
                })
                .into_grouping_map()
                .collect();

        let adj_processor: HashMap<_, _> = processors
            .clone()
            .flatten()
            .map(|proc_ident| {
                let sinks = proc_to_sinks.remove(&*proc_ident);
                let srcs = proc_to_srcs.remove(&*proc_ident);
                let inout = ProcInOut {
                    inputs: srcs,
                    outputs: sinks,
                };

                (proc_ident, inout)
            })
            .collect();

        Ok(Self {
            bindings,
            processors,
            exchanges,
            adj_exchange,
            adj_processor,
            config,
        })
    }

    /// Save the dataflow graph to the GraphViz DOT file.
    pub fn save_dot_file<F>(&self, file: F) -> io::Result<()>
    where
        F: AsRef<Path>,
    {
        let mut writer = BufWriter::new(File::create(file)?);
        dot::render(self, &mut writer)?;
        Ok(())
    }

    /// Get the names of declared processors.
    pub fn processors(&self) -> &IndexSet<Ident> {
        &self.processors
    }

    /// Get the names and configurations of declared exchanges.
    pub fn exchanges(&self) -> &IndexMap<Key, Exchange> {
        &self.exchanges
    }

    /// Get the file bindings for each exchange namespace.
    pub fn bindings(&self) -> &IndexMap<PathBuf, Dir> {
        &self.bindings
    }

    /// Build a receiver to an exhcnage for the single-input processor `proc`.
    ///
    /// The processor is implicitly assumed to connect to one input
    /// exchange. If not, the method returns an error. For the
    /// multi-input processor, use
    /// [build_receiver_from](Graph::build_receiver_from) instead.
    pub async fn build_receiver<N>(&self, proc: N) -> Result<easyflow_link::Receiver, Error>
    where
        N: IntoIdent,
    {
        let proc = proc.into();
        let inputs = self
            .adj_processor
            .get(&proc)
            .ok_or_else(|| Error::processor_not_found(&proc))?
            .inputs
            .as_ref()
            .ok_or_else(|| Error::no_input_available(&proc))?;
        let exchange = {
            let mut iter = inputs.iter();
            let exg = iter
                .next()
                .ok_or_else(|| Error::no_input_available(&proc))?;

            if iter.next().is_some() {
                return Err(Error::input_not_specified(&proc));
            }
            exg
        };
        let receiver = self.exchanges[&**exchange].build_receiver().await?;
        Ok(receiver)
    }

    /// Build a sender to an exhcnage for the single-output processor `proc`.
    ///
    /// The processor is implicitly assumed to connect to one output
    /// exchange. If not, the method returns an error. For the
    /// multi-output processor, use
    /// [build_sender_to](Graph::build_sender_to) instead.
    pub async fn build_sender<N>(&self, proc: N) -> Result<easyflow_link::Sender, Error>
    where
        N: IntoIdent,
    {
        let proc = proc.into();
        let outputs = self
            .adj_processor
            .get(&proc)
            .ok_or_else(|| Error::processor_not_found(&proc))?
            .outputs
            .as_ref()
            .ok_or_else(|| Error::no_output_available(&proc))?;

        let exchange = {
            let mut iter = outputs.iter();
            let exg = iter
                .next()
                .ok_or_else(|| Error::no_output_available(&proc))?;

            if iter.next().is_some() {
                return Err(Error::output_not_specified(&proc));
            }
            exg
        };

        let sender = self.exchanges[&**exchange].build_sender().await?;

        Ok(sender)
    }

    /// Build a receiver to the `exhcnage` for the processor `proc`.
    pub async fn build_receiver_from<N, E>(
        &self,
        proc: N,
        exchange: E,
    ) -> Result<easyflow_link::Receiver, Error>
    where
        N: IntoIdent,
        E: IntoKey,
    {
        let proc = proc.into();
        let exchange = exchange.into();
        let inputs = self
            .adj_processor
            .get(&proc)
            .ok_or_else(|| Error::processor_not_found(&proc))?
            .inputs
            .as_ref()
            .ok_or_else(|| Error::connection_error(&proc, &exchange))?;
        let key = inputs
            .get(&exchange)
            .ok_or_else(|| Error::connection_error(&proc, &exchange))?;
        let receiver = self.exchanges[&**key].build_receiver().await?;
        Ok(receiver)
    }

    /// Build a sender to the `exhcnage` for the processor `proc`.
    pub async fn build_sender_to<N, E>(
        &self,
        proc: N,
        exchange: E,
    ) -> Result<easyflow_link::Sender, Error>
    where
        N: IntoIdent,
        E: IntoKey,
    {
        let proc = proc.into();
        let exchange = exchange.into();

        let outputs = self
            .adj_processor
            .get(&proc)
            .ok_or_else(|| Error::processor_not_found(&proc))?
            .outputs
            .as_ref()
            .ok_or_else(|| Error::connection_error(&proc, &exchange))?;
        let key = outputs
            .get(&exchange)
            .ok_or_else(|| Error::connection_error(&proc, &exchange))?;
        let sender = self.exchanges[&**key].build_sender().await?;
        Ok(sender)
    }
}

impl TryFrom<GraphConfig> for Dataflow {
    type Error = anyhow::Error;

    fn try_from(config: GraphConfig) -> Result<Self, Self::Error> {
        Dataflow::from_config(config)
    }
}

impl From<Dataflow> for GraphConfig {
    fn from(graph: Dataflow) -> Self {
        graph.config
    }
}

mod graphviz {
    use super::*;
    use dot::{Edges, GraphWalk, Id, LabelText, Labeller, Nodes};
    use std::borrow::Cow;

    #[derive(Clone)]
    pub(crate) enum Node<'a> {
        Processor(&'a Ident),
        Exchange(&'a Key),
    }

    #[derive(Clone)]
    pub(crate) enum Edge<'a> {
        SrcIn { src: &'a Key, input: &'a Ident },
        SinkOut { sink: &'a Key, output: &'a Ident },
    }

    impl<'a> Labeller<'a, Node<'a>, Edge<'a>> for Dataflow {
        fn graph_id(&'a self) -> Id<'a> {
            Id::new("flow_graph").unwrap()
        }

        fn node_id(&'a self, node: &Node<'a>) -> Id<'a> {
            let name = match node {
                Node::Processor(proc) => proc.to_string(),
                Node::Exchange(exg) => exg.to_string(),
            };
            Id::new(name.replace(&['-', '/'][..], "_")).unwrap()
        }

        fn node_label(&'a self, node: &Node<'a>) -> LabelText<'a> {
            let name = match node {
                Node::Processor(proc) => proc.to_string(),
                Node::Exchange(exg) => exg.to_string(),
            };
            LabelText::LabelStr(Into::into(name))
        }

        fn node_shape(&'a self, node: &Node<'a>) -> Option<LabelText<'a>> {
            let shape = match node {
                Node::Processor(_) => LabelText::LabelStr(Into::into("box")),
                Node::Exchange(_) => LabelText::LabelStr(Into::into("hexagon")),
            };
            Some(shape)
        }
    }

    impl<'a> GraphWalk<'a, Node<'a>, Edge<'a>> for Dataflow {
        fn nodes(&'a self) -> Nodes<'a, Node<'a>> {
            let processors = self.processors.iter().map(Node::Processor);
            let exchanges = self.exchanges.keys().map(Node::Exchange);
            let nodes: Vec<_> = chain!(processors, exchanges).collect();
            Cow::Owned(nodes)
        }

        fn edges(&'a self) -> Edges<'a, Edge<'a>> {
            let edges: Vec<_> = self
                .adj_exchange
                .iter()
                .flat_map(|(exchange, conn)| {
                    let sink_edges = conn.sink_iter().map(|proc| Edge::SinkOut {
                        sink: exchange,
                        output: proc,
                    });
                    let src_edges = conn.source_iter().map(|proc| Edge::SrcIn {
                        src: exchange,
                        input: proc,
                    });
                    chain!(sink_edges, src_edges)
                })
                .collect();

            Cow::Owned(edges)
        }

        fn source(&'a self, edge: &Edge) -> Node<'a> {
            match edge {
                Edge::SinkOut { output, .. } => {
                    Node::Processor(self.processors.get(*output).unwrap())
                }
                Edge::SrcIn { src, .. } => {
                    Node::Exchange(self.exchanges.get_key_value(*src).unwrap().0)
                }
            }
        }

        fn target(&'a self, edge: &Edge) -> Node<'a> {
            match edge {
                Edge::SinkOut { sink, .. } => {
                    Node::Exchange(self.exchanges.get_key_value(*sink).unwrap().0)
                }
                Edge::SrcIn { input, .. } => Node::Processor(self.processors.get(*input).unwrap()),
            }
        }
    }
}
