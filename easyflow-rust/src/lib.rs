//! Dataflow configuration and connection setup.
//!
//! The crate defines configuration format to describe the dataflow of
//! *processors* and the *connections*. The configuration can be
//! deserialized into a [Graph]. It is used to open data senders and
//! receivers for each processor in the dataflow.  It can be
//! deserialized into [Graph].
//!
//! # Dataflow Configuration Format
//!
//! The dataflow configuratoin file is written in JSON5 format,
//! declaring the following primitives:
//!
//! - **Processor**: The program with zero, one or more inputs and outputs.
//! - **Exchange**: It connects the inputs of one or more processors to the outputs of the other processors.
//!
//! ```json5
//! {
//!     // The version number is mandatory.
//!     "version": "0.1.0",
//!
//!     // The list of processor names.
//!     "processors": [
//!         "background-remove",
//!     ],
//!
//!     // The list of exchanges and respective parameters.
//!     "exchanges": {
//!         "file-pcd-process": {
//!             "type": "file",
//!             "dir": "pcd-process_output"
//!         },
//!         "file-background-remove": {
//!             "type": "file",
//!             "dir": "background-remove_output"
//!         },
//!
//!     },
//!
//!     // Declares the connected processor inputs and outputs for each exchanges.
//!     "connections": {
//!         "file-pcd-process": {
//!             ">": ["background-remove"],
//!         },
//!         "file-background-remove": {
//!             "<": ["background-remove"],
//!         },
//!     },
//! }
//! ```
//!
//! # Usage
//!
//! The code example loads the dataflow graph from the configuration
//! file above, and build and sender and a receiver to demonstrate
//! data transfer.
//!
//! ```skip
//! # // fixme: fix this test
//! # futures::executor::block_on(async {
//! use dataflow_config::Graph;
//!
//! // Open the dataflow graph
//! let file = concat!(env!("CARGO_MANIFEST_DIR"), "/config-examples/single.json5");
//! let graph = Graph::open(file)?;
//!
//! let data: &[u8] = &[3, 1, 4, 1, 5, 9];
//!
//! // Create a sender for "background-remove" processor
//! let mut sender = graph.build_sender("background-remove").await?;
//! sender.send(data).await?;
//!
//! // Create a receiver for "background-remove" processor
//! let mut receiver = graph.build_receiver("background-remove").await?;
//! let received = receiver.recv().await?;
//!
//! assert!(received.unwrap() == data);
//! # anyhow::Ok(())
//! # });
//! ```

mod dataflow;
mod error;

pub use dataflow::*;
pub use error::Error;
