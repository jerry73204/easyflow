use easyflow_config::{Ident, IntoIdent, IntoKey, Key};
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("processor `{processor}` not found")]
    ProcessorNotFound { processor: Ident },
    #[error("exchange `{exchange}` not found")]
    ExchangeNotFound { exchange: Key },
    #[error("processor `{processor}` has no inputs")]
    NoInputAvailable { processor: Ident },
    #[error("processor `{processor}` has no outputs")]
    NoOutputAvailable { processor: Ident },
    #[error("the exchange `{exchange}` is not connected to the processor `{processor}`")]
    ConnectionError { processor: Ident, exchange: Key },
    #[error("the input exchange to processor `{processor}` must be specified")]
    InputNotSpecified { processor: Ident },
    #[error("the output exchange from processor `{processor}` must be specified")]
    OutputNotSpecified { processor: Ident },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("internal error: {0}")]
    Other(#[from] anyhow::Error),
}

impl Error {
    pub fn processor_not_found(processor: impl IntoIdent) -> Self {
        Self::ProcessorNotFound {
            processor: processor.into(),
        }
    }

    pub fn exchange_not_found(exchange: impl IntoKey) -> Self {
        Self::ExchangeNotFound {
            exchange: exchange.into(),
        }
    }
    pub fn no_input_available(processor: impl IntoIdent) -> Self {
        Self::NoInputAvailable {
            processor: processor.into(),
        }
    }

    pub fn no_output_available(processor: impl IntoIdent) -> Self {
        Self::NoOutputAvailable {
            processor: processor.into(),
        }
    }

    pub fn input_not_specified(processor: impl IntoIdent) -> Self {
        Self::InputNotSpecified {
            processor: processor.into(),
        }
    }

    pub fn output_not_specified(processor: impl IntoIdent) -> Self {
        Self::OutputNotSpecified {
            processor: processor.into(),
        }
    }

    pub fn connection_error(processor: impl IntoIdent, exchange: impl IntoKey) -> Self {
        Self::ConnectionError {
            processor: processor.into(),
            exchange: exchange.into(),
        }
    }
}
